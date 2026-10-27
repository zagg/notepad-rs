//! Documents = tabs. The RichEdit control holds the live text; each Doc caches
//! its content when it is not the active tab.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH, WM_SETTEXT};

use crate::encoding::{self, Enc, Eol};
use crate::state::app;
use notepad_rs::richedit::{EM_EMPTYUNDOBUFFER, EM_SETMODIFY, EM_SETREADONLY};

#[derive(Clone)]
pub struct Doc {
    pub path: Option<PathBuf>,
    pub text: String,
    pub eol: Eol,
    pub enc: Enc,
    pub modified: bool,
    pub readonly: bool,
}

impl Doc {
    pub fn new_untitled(eol: Eol) -> Self {
        Self {
            path: None,
            text: String::new(),
            eol,
            enc: Enc::Utf8,
            modified: false,
            readonly: false,
        }
    }

    /// Tab caption (Untitled / file name), "*" when dirty (observed: "Untitled", "*Untitled*").
    pub fn title(&self) -> String {
        let name = match &self.path {
            Some(p) => p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string()),
            None => "Sans titre".to_string(),
        };
        if self.modified {
            format!("*{name}")
        } else {
            name
        }
    }

    pub fn window_title(&self) -> String {
        let name = match &self.path {
            Some(p) => p.display().to_string(),
            None => "Sans titre".to_string(),
        };
        format!("{} - Bloc-notes", name)
    }
}

/// Pull the current control content into the active Doc.
pub fn sync_from_control() {
    let a = app();
    let text = get_control_text(a.edit);
    if let Some(d) = a.docs.get_mut(a.cur) {
        if d.text != text {
            d.text = text;
        }
    }
}

pub fn get_control_text(edit: HWND) -> String {
    unsafe {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        let len = SendMessageW(edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as usize;
        let mut buf = vec![0u16; len + 1];
        let n = SendMessageW(edit, WM_GETTEXT, WPARAM(len + 1), LPARAM(buf.as_mut_ptr() as isize)).0;
        let s = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        // RichEdit reports \r as line separator; normalize to \n.
        s.replace('\r', "\n")
    }
}

pub fn set_control_text(edit: HWND, text: &str) {
    let w: Vec<u16> = text.replace('\n', "\r").encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        SendMessageW(edit, WM_SETTEXT, WPARAM(0), LPARAM(w.as_ptr() as isize));
        SendMessageW(edit, EM_EMPTYUNDOBUFFER, WPARAM(0), LPARAM(0));
        SendMessageW(edit, EM_SETMODIFY, WPARAM(0), LPARAM(0));
    }
}

/// Create a new tab (Ctrl+N): sync current, push untitled doc, show it.
pub fn new_tab() {
    sync_from_control();
    let a = app();
    let eol = if a.settings.windows_eol { Eol::CrLf } else { Eol::Lf };
    a.docs.push(Doc::new_untitled(eol));
    a.cur = a.docs.len() - 1;
    crate::app::refresh_tabs();
    show_current();
    set_modified_flag(false);
}

pub fn open_path(path: &Path) -> bool {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let (enc, text) = encoding::decode(&data);
    let eol = encoding::detect_eol(&data);
    let a = app();
    // If the active tab is a pristine empty untitled doc, reuse it.
    let reuse = a.docs.len() == 1
        && a.cur == 0
        && a.docs[0].path.is_none()
        && a.docs[0].text.is_empty()
        && !a.docs[0].modified;
    let doc = Doc {
        path: Some(path.to_path_buf()),
        text,
        eol,
        enc,
        modified: false,
        readonly: !path.as_os_str().is_empty() && !path_can_write(path),
    };
    if reuse {
        a.docs[0] = doc;
    } else {
        sync_from_control();
        a.docs.push(doc);
        a.cur = a.docs.len() - 1;
    }
    crate::app::refresh_tabs();
    show_current();
    set_modified_flag(false);
    true
}

fn path_can_write(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| !m.permissions().readonly()).unwrap_or(true)
}

pub fn save_current(as_new: bool) -> bool {
    let a = app();
    sync_from_control();
    let idx = a.cur;
    let doc = &a.docs[idx];
    let target: PathBuf = if as_new || doc.path.is_none() {
        match prompt_save_as(doc) {
            Some(p) => p,
            None => return false,
        }
    } else {
        doc.path.clone().unwrap()
    };
    let (text, eol, enc) = {
        let d = &a.docs[idx];
        (d.text.clone(), d.eol, d.enc)
    };
    let bytes = encoding::encode(&text, enc, eol);
    if std::fs::write(&target, &bytes).is_err() {
        return false;
    }
    let a = app();
    a.docs[idx].path = Some(target);
    a.docs[idx].modified = false;
    crate::app::refresh_tabs();
    crate::app::update_title();
    set_modified_flag(false);
    true
}

fn prompt_save_as(doc: &Doc) -> Option<PathBuf> {
    crate::dialogs::save_as_dialog(doc.path.as_deref())
}

pub fn show_current() {
    let a = app();
    let text = a.docs[a.cur].text.clone();
    let readonly = a.docs[a.cur].readonly;
    set_control_text(a.edit, &text);
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    unsafe {
        SendMessageW(a.edit, EM_SETREADONLY, WPARAM(readonly as usize), LPARAM(0));
    }
    crate::app::update_title();
    crate::status::update();
}

pub fn switch_to(index: usize) {
    let a = app();
    if index >= a.docs.len() {
        return;
    }
    sync_from_control();
    a.cur = index;
    show_current();
}

pub fn close_tab(index: usize, prompt: bool) -> bool {
    sync_from_control();
    let a = app();
    if index >= a.docs.len() {
        return false;
    }
    if prompt && a.docs[index].modified {
        let name = a.docs[index].title();
        if !crate::dialogs::confirm_save(&name) {
            return false; // Cancel -> abort close
        }
        // "Don't save" already handled inside confirm_save
    }
    let a = app();
    a.docs.remove(index);
    if a.docs.is_empty() {
        a.docs.push(Doc::new_untitled(if a.settings.windows_eol { Eol::CrLf } else { Eol::Lf }));
    }
    if a.cur >= a.docs.len() {
        a.cur = a.docs.len() - 1;
    }
    crate::app::refresh_tabs();
    show_current();
    true
}

pub fn mark_modified() {
    let a = app();
    if let Some(d) = a.docs.get_mut(a.cur) {
        if !d.modified {
            d.modified = true;
            crate::app::update_title();
            crate::app::refresh_tabs();
        }
    }
}

fn set_modified_flag(modified: bool) {
    let a = app();
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    unsafe {
        SendMessageW(a.edit, EM_SETMODIFY, WPARAM(modified as usize), LPARAM(0));
    }
}
