//! Non-modal Find / Replace windows.
//! Evidence keys: FindReplaceDialogViewModel, FREViewModel, "CannotFindString",
//! "AllItemsReplaced", "LastItemReplaced", "WrappedBegin/WrappedEnd".

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetSysColorBrush, COLOR_BTNFACE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::state::app;
use notepad_rs::richedit::{
    CHARRANGE, EM_EXGETSEL, EM_EXSETSEL, EM_REPLACESEL, EM_SCROLLCARET,
};

/// Read the current selection as (start, end) character positions through
/// EM_EXGETSEL (CHARRANGE): full 32-bit positions, RichEdit-native.
fn get_sel() -> (u32, u32) {
    let a = app();
    unsafe {
        let mut cr = CHARRANGE::default();
        SendMessageW(a.edit, EM_EXGETSEL, WPARAM(0), LPARAM(&mut cr as *mut _ as isize));
        (cr.cp_min.max(0) as u32, cr.cp_max.max(0) as u32)
    }
}

/// Set the selection through EM_EXSETSEL and bring it into view.
fn set_sel(start: usize, end: usize) {
    let a = app();
    unsafe {
        let cr = CHARRANGE {
            cp_min: start as i32,
            cp_max: end as i32,
        };
        SendMessageW(a.edit, EM_EXSETSEL, WPARAM(0), LPARAM(&cr as *const _ as isize));
        SendMessageW(a.edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    }
}

use notepad_rs::wf::{child_id, pcw, to_wide};

const IDC_FIND_EDIT: usize = 1001;
const IDC_FIND_CASE: usize = 1002;
const IDC_FIND_WORD: usize = 1003;
const IDC_FIND_WRAP: usize = 1004;
const IDC_FIND_NEXT: usize = 1005;
const IDC_FIND_CANCEL: usize = 1006;

const IDC_REP_FIND: usize = 2001;
const IDC_REP_WITH: usize = 2002;
const IDC_REP_CASE: usize = 2003;
const IDC_REP_WORD: usize = 2004;
const IDC_REP_WRAP: usize = 2005;
const IDC_REP_DO: usize = 2006;
const IDC_REP_ALL: usize = 2007;
const IDC_REP_CANCEL: usize = 2008;

pub fn open_find(parent: HWND) {
    let a = app();
    if !a.find.hwnd_find.is_invalid() && unsafe { IsWindow(a.find.hwnd_find).as_bool() } {
        unsafe {
            let _ = SetForegroundWindow(a.find.hwnd_find);
        }
        return;
    }
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap_or_default().into() };
    let class = to_wide("NotepadFind");
    register_class(&class, find_proc, hinst);
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&class),
            pcw(&to_wide("Rechercher")),
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT, CW_USEDEFAULT, 380, 150,
            parent,
            HMENU::default(),
            hinst,
            None,
        )
        .unwrap_or_default()
    };
    app().find.hwnd_find = hwnd;
    // Pre-fill with current selection when it is a single line.
    let sel = current_selection();
    if !sel.is_empty() && !sel.contains('\n') {
        set_edit_text(get_child(hwnd, IDC_FIND_EDIT), &sel);
        app().find.text = sel;
    }
    create_children_find(hwnd, hinst);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
}

fn create_children_find(hwnd: HWND, hinst: HINSTANCE) {
    let button = to_wide("BUTTON");
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            pcw(&to_wide("EDIT")),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
            96, 8, 250, 22,
            hwnd,
            child_id(IDC_FIND_EDIT),
            hinst,
            None,
        )
        .ok();
        let mut x = 96i32;
        for (id, label) in [
            (IDC_FIND_CASE, "Respecter la casse"),
            (IDC_FIND_WORD, "Mot entier"),
            (IDC_FIND_WRAP, "Boucler"),
        ] {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                pcw(&button),
                pcw(&to_wide(label)),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                x, 40, 84, 20,
                hwnd,
                child_id(id),
                hinst,
                None,
            )
            .ok();
            x += 90;
        }
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&button),
            pcw(&to_wide("&Suivant")),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            96, 74, 90, 26,
            hwnd,
            child_id(IDC_FIND_NEXT),
            hinst,
            None,
        )
        .ok();
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&button),
            pcw(&to_wide("Annuler")),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            200, 74, 90, 26,
            hwnd,
            child_id(IDC_FIND_CANCEL),
            hinst,
            None,
        )
        .ok();
    }
}

unsafe extern "system" fn find_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (w.0 & 0xFFFF) as usize;
            match id {
                IDC_FIND_NEXT | 1 => {
                    // 1 = VK_RETURN via default button
                    read_find_controls(hwnd);
                    let ok = find_next(true);
                    if !ok {
                        not_found();
                    }
                    LRESULT(0)
                }
                IDC_FIND_CANCEL | 2 => {
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, w, l),
            }
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let a = app();
            a.find.hwnd_find = HWND::default();
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

pub fn open_replace(parent: HWND) {
    let a = app();
    if !a.find.hwnd_replace.is_invalid() && unsafe { IsWindow(a.find.hwnd_replace).as_bool() } {
        unsafe {
            let _ = SetForegroundWindow(a.find.hwnd_replace);
        }
        return;
    }
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap_or_default().into() };
    let class = to_wide("NotepadReplace");
    register_class(&class, replace_proc, hinst);
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&class),
            pcw(&to_wide("Remplacer")),
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 200,
            parent,
            HMENU::default(),
            hinst,
            None,
        )
        .unwrap_or_default()
    };
    app().find.hwnd_replace = hwnd;
    let sel = current_selection();
    if !sel.is_empty() && !sel.contains('\n') {
        set_edit_text(get_child(hwnd, IDC_REP_FIND), &sel);
        app().find.text = sel;
    }
    let button = to_wide("BUTTON");
    unsafe {
        CreateWindowExW(WS_EX_CLIENTEDGE, pcw(&to_wide("EDIT")), PCWSTR::null(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER, 130, 8, 260, 22, hwnd, child_id(IDC_REP_FIND), hinst, None).ok();
        CreateWindowExW(WS_EX_CLIENTEDGE, pcw(&to_wide("EDIT")), PCWSTR::null(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER, 130, 36, 260, 22, hwnd, child_id(IDC_REP_WITH), hinst, None).ok();
        let mut x = 130i32;
        for (id, label) in [(IDC_REP_CASE, "Respecter la casse"), (IDC_REP_WORD, "Mot entier"), (IDC_REP_WRAP, "Boucler")] {
            CreateWindowExW(WINDOW_EX_STYLE(0), pcw(&button), pcw(&to_wide(label)), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32), x, 68, 84, 20, hwnd, child_id(id), hinst, None).ok();
            x += 90;
        }
        CreateWindowExW(WINDOW_EX_STYLE(0), pcw(&button), pcw(&to_wide("&Remplacer")), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32), 130, 100, 90, 26, hwnd, child_id(IDC_REP_DO), hinst, None).ok();
        CreateWindowExW(WINDOW_EX_STYLE(0), pcw(&button), pcw(&to_wide("&Tout remplacer")), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), 226, 100, 90, 26, hwnd, child_id(IDC_REP_ALL), hinst, None).ok();
        CreateWindowExW(WINDOW_EX_STYLE(0), pcw(&button), pcw(&to_wide("Annuler")), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32), 322, 100, 68, 26, hwnd, child_id(IDC_REP_CANCEL), hinst, None).ok();
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
}

unsafe extern "system" fn replace_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (w.0 & 0xFFFF) as usize;
            match id {
                IDC_REP_DO | 1 => {
                    read_replace_controls(hwnd);
                    replace_next();
                    LRESULT(0)
                }
                IDC_REP_ALL => {
                    read_replace_controls(hwnd);
                    replace_all();
                    LRESULT(0)
                }
                IDC_REP_CANCEL | 2 => {
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, w, l),
            }
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let a = app();
            a.find.hwnd_replace = HWND::default();
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

// ---------------------------------------------------------------- controls

fn register_class(
    class: &[u16],
    proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    hinst: HINSTANCE,
) {
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(proc),
        hInstance: hinst,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
        hbrBackground: unsafe { GetSysColorBrush(COLOR_BTNFACE) },
        lpszClassName: pcw(class),
        ..Default::default()
    };
    unsafe {
        RegisterClassExW(&wc);
    }
}

fn get_child(parent: HWND, id: usize) -> HWND {
    unsafe { GetDlgItem(parent, id as i32).unwrap_or_default() }
}

fn get_edit_text(hwnd: HWND) -> String {
    unsafe {
        let len = SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as usize;
        let mut buf = vec![0u16; len + 1];
        SendMessageW(hwnd, WM_GETTEXT, WPARAM(len + 1), LPARAM(buf.as_mut_ptr() as isize));
        let n = buf.iter().position(|&c| c == 0).unwrap_or(len);
        String::from_utf16_lossy(&buf[..n])
    }
}

fn set_edit_text(hwnd: HWND, text: &str) {
    let w = to_wide(text);
    unsafe {
        SendMessageW(hwnd, WM_SETTEXT, WPARAM(0), LPARAM(w.as_ptr() as isize));
    }
}

fn is_checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 != 0 }
}

fn read_find_controls(hwnd: HWND) {
    let a = app();
    a.find.text = get_edit_text(get_child(hwnd, IDC_FIND_EDIT));
    a.find.match_case = is_checked(get_child(hwnd, IDC_FIND_CASE));
    a.find.whole_word = is_checked(get_child(hwnd, IDC_FIND_WORD));
    a.find.wrap_around = is_checked(get_child(hwnd, IDC_FIND_WRAP));
}

fn read_replace_controls(hwnd: HWND) {
    let a = app();
    a.find.text = get_edit_text(get_child(hwnd, IDC_REP_FIND));
    a.find.replace = get_edit_text(get_child(hwnd, IDC_REP_WITH));
    a.find.match_case = is_checked(get_child(hwnd, IDC_REP_CASE));
    a.find.whole_word = is_checked(get_child(hwnd, IDC_REP_WORD));
    a.find.wrap_around = is_checked(get_child(hwnd, IDC_REP_WRAP));
}

fn current_selection() -> String {
    let a = app();
    let (s, e) = get_sel();
    if s >= e {
        return String::new();
    }
    let text = crate::document::get_control_text(a.edit);
    let chars: Vec<char> = text.chars().collect();
    chars[s as usize..e as usize].iter().collect()
}

fn not_found() {
    let a = app();
    unsafe {
        MessageBoxW(
            a.hwnd,
            pcw(&to_wide("Impossible de trouver le texte recherché.")),
            pcw(&to_wide("Bloc-notes")),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

// ---------------------------------------------------------------- search core

/// Search from the caret; returns true when found (selection moved there).
pub fn find_next(fwd: bool) -> bool {
    let a = app();
    if a.find.text.is_empty() {
        return false;
    }
    let text = crate::document::get_control_text(a.edit);
    let (s, e) = get_sel();
    let (s, e) = (s as usize, e as usize);
    let (start, end) = if s <= e { (s, e) } else { (e, s) };

    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = a.find.text.chars().collect();
    let eq = |x: char, y: char| {
        if a.find.match_case {
            x == y
        } else {
            x.to_lowercase().eq(y.to_lowercase())
        }
    };
    let word_ok = |pos: usize, len: usize| -> bool {
        if !a.find.whole_word {
            return true;
        }
        let before_ok = pos == 0 || !hay[pos - 1].is_alphanumeric();
        let after_ok = pos + len >= hay.len() || !hay[pos + len].is_alphanumeric();
        before_ok && after_ok
    };

    let n = hay.len();
    let ml = needle.len();
    let mut found: Option<usize> = None;
    if fwd {
        let from = end.min(n);
        for pos in from..=(n.saturating_sub(ml)) {
            if pos + ml > n {
                break;
            }
            if hay[pos..pos + ml].iter().zip(needle.iter()).all(|(a, b)| eq(*a, *b))
                && word_ok(pos, ml)
            {
                found = Some(pos);
                break;
            }
        }
        if found.is_none() && a.find.wrap_around {
            for pos in 0..end.min(n) {
                if pos + ml > n {
                    break;
                }
                if hay[pos..pos + ml].iter().zip(needle.iter()).all(|(a, b)| eq(*a, *b))
                    && word_ok(pos, ml)
                {
                    found = Some(pos);
                    break;
                }
            }
        }
    } else {
        let to = start.min(n);
        for pos in (0..=to.saturating_sub(1)).rev() {
            if pos + ml > n {
                continue;
            }
            if hay[pos..pos + ml].iter().zip(needle.iter()).all(|(a, b)| eq(*a, *b))
                && word_ok(pos, ml)
            {
                found = Some(pos);
                break;
            }
        }
        if found.is_none() && a.find.wrap_around {
            let from = n.saturating_sub(ml);
            for pos in (start.min(n)..=from).rev() {
                if pos + ml > n {
                    continue;
                }
                if hay[pos..pos + ml].iter().zip(needle.iter()).all(|(a, b)| eq(*a, *b))
                    && word_ok(pos, ml)
                {
                    found = Some(pos);
                    break;
                }
            }
        }
    }

    match found {
        Some(pos) => {
            set_sel(pos, pos + ml);
            crate::status::update();
            true
        }
        None => false,
    }
}

pub fn replace_next() {
    // If current selection matches the needle, replace it first.
    let a = app();
    if a.find.text.is_empty() {
        return;
    }
    let text = crate::document::get_control_text(a.edit);
    let (s, e) = get_sel();
    let (s, e) = ((s as usize).min((e as usize).max(s as usize)), (e as usize));
    let (s, e) = if s <= e { (s, e) } else { (e, s) };
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = a.find.text.chars().collect();
    let eq = |x: char, y: char| {
        if a.find.match_case {
            x == y
        } else {
            x.to_lowercase().eq(y.to_lowercase())
        }
    };
    let matched = s != e
        && (e - s) == needle.len()
        && chars[s..e].iter().zip(needle.iter()).all(|(a, b)| eq(*a, *b));
    let edit = a.edit;
    if matched {
        let rep: Vec<u16> = a
            .find
            .replace
            .replace('\n', "\r")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            SendMessageW(edit, EM_REPLACESEL, WPARAM(1), LPARAM(rep.as_ptr() as isize));
        }
    }
    if !find_next(true) {
        not_found();
    }
}

pub fn replace_all() {
    let a = app();
    if a.find.text.is_empty() {
        return;
    }
    let edit = a.edit;
    // Move caret to start.
    set_sel(0, 0);
    let mut count = 0usize;
    while find_next(true) {
        let rep: Vec<u16> = a
            .find
            .replace
            .replace('\n', "\r")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            SendMessageW(edit, EM_REPLACESEL, WPARAM(1), LPARAM(rep.as_ptr() as isize));
        }
        count += 1;
        if count > 500_000 {
            break;
        }
    }
}

/// F3 / Ctrl+H entry point from the app.
pub fn find_next_key(shift: bool) {
    let a = app();
    if a.find.text.is_empty() {
        // Nothing stored: open the dialog instead.
        open_find(a.hwnd);
        return;
    }
    if !find_next(!shift) {
        not_found();
    }
}
