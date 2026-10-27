//! File dialogs (comdlg32 via the windows crate), Go To, Font, Page Setup,
//! save confirmation and About.
//! Evidence keys: FileOpenDialogTitle, FileSaveDialogTitle, FileOpenAllFilesFilter,
//! filter pattern "%s%c*.txt%c%s%c*.*%c" (Tous les fichiers + *.txt).

use std::path::PathBuf;

use windows::core::{PWSTR, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetSysColorBrush, COLOR_BTNFACE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    ChooseFontW, GetOpenFileNameW, GetSaveFileNameW, PageSetupDlgW, CF_EFFECTS,
    CF_INITTOLOGFONTSTRUCT, CF_SCREENFONTS, CHOOSEFONTW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW, PAGESETUPDLGW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::state::app;
use notepad_rs::wf::{child_id, pcw, to_wide};

const FILTER: &str = "Documents texte (*.txt)\0*.txt\0Tous les fichiers (*.*)\0*.*\0\0";

fn base_ofn(buf: &mut Vec<u16>, title: &str, initial: Option<&std::path::Path>) -> OPENFILENAMEW {
    let mut ofn = OPENFILENAMEW::default();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = app().hwnd;
    ofn.hInstance = unsafe { GetModuleHandleW(None).unwrap_or_default().into() };
    ofn.lpstrFilter = PCWSTR(FILTER.encode_utf16().collect::<Vec<u16>>().leak().as_ptr());
    ofn.lpstrFile = PWSTR(buf.as_mut_ptr());
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrTitle = PCWSTR(to_wide(title).leak().as_ptr());
    ofn.lpstrDefExt = PCWSTR(to_wide("txt").leak().as_ptr());
    ofn.Flags = OFN_EXPLORER;
    if let Some(p) = initial {
        let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
        ofn.lpstrInitialDir = PCWSTR(to_wide(&dir).leak().as_ptr());
        let name = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let mut off = 0;
        for c in name.encode_utf16() {
            if off + 1 < buf.len() {
                buf[off] = c;
                off += 1;
            }
        }
        buf[off] = 0;
    }
    ofn
}

pub fn open_dialog() -> Option<PathBuf> {
    let mut buf = vec![0u16; 32768];
    let mut ofn = base_ofn(&mut buf, "Ouvrir", None);
    ofn.Flags |= OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;
    if unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        Some(PathBuf::from(String::from_utf16_lossy(
            &buf[..buf.iter().position(|&c| c == 0)?],
        )))
    } else {
        None
    }
}

pub fn save_as_dialog(current: Option<&std::path::Path>) -> Option<PathBuf> {
    let mut buf = vec![0u16; 32768];
    let mut ofn = base_ofn(&mut buf, "Enregistrer sous", current);
    ofn.Flags |= OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;
    if unsafe { GetSaveFileNameW(&mut ofn) }.as_bool() {
        Some(PathBuf::from(String::from_utf16_lossy(
            &buf[..buf.iter().position(|&c| c == 0)?],
        )))
    } else {
        None
    }
}

/// "Voulez-vous enregistrer les modifications ?" -> true means "continue with close".
pub fn confirm_save(name: &str) -> bool {
    let a = app();
    let r = unsafe {
        MessageBoxW(
            a.hwnd,
            pcw(&to_wide(&format!(
                "Voulez-vous enregistrer les modifications apportées à {name} ?"
            ))),
            pcw(&to_wide("Bloc-notes")),
            MB_YESNOCANCEL | MB_ICONWARNING,
        )
    };
    match r {
        IDYES => crate::document::save_current(false),
        IDNO => true,
        _ => false,
    }
}

pub fn about_box() {
    let a = app();
    unsafe {
        MessageBoxW(
            a.hwnd,
            pcw(&to_wide(
                "Bloc-notes (clone Rust)\n\n\
                 Clone de C:\\Windows\\notepad.exe realise en Rust pur (Win32).\n\
                 Stub + editeur complet, sans application du Store.",
            )),
            pcw(&to_wide("À propos du Bloc-notes")),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Modal "Atteindre" dialog (GotoDialogContentViewModel evidence).
pub fn goto_dialog() -> Option<u32> {
    let a = app();
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap_or_default().into() };
    let class = to_wide("NotepadGoto");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(goto_proc),
        hInstance: hinst,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
        hbrBackground: unsafe { GetSysColorBrush(COLOR_BTNFACE) },
        lpszClassName: pcw(&class),
        ..Default::default()
    };
    unsafe {
        RegisterClassExW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&class),
            pcw(&to_wide("Atteindre")),
            WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT, CW_USEDEFAULT, 300, 150,
            a.hwnd,
            HMENU::default(),
            hinst,
            None,
        )
        .unwrap_or_default();
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            pcw(&to_wide("EDIT")),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
            150, 14, 120, 22,
            hwnd,
            child_id(3001),
            hinst,
            None,
        )
        .ok();
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&to_wide("Static")),
            pcw(&to_wide("Numéro de ligne :")),
            WS_CHILD | WS_VISIBLE,
            14, 16, 120, 20,
            hwnd,
            child_id(3002),
            hinst,
            None,
        )
        .ok();
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&to_wide("Button")),
            pcw(&to_wide("OK")),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            60, 60, 80, 26,
            hwnd,
            child_id(3003),
            hinst,
            None,
        )
        .ok();
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&to_wide("Button")),
            pcw(&to_wide("Annuler")),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            150, 60, 80, 26,
            hwnd,
            child_id(3004),
            hinst,
            None,
        )
        .ok();
        let _ = ShowWindow(hwnd, SW_SHOW);

        MODAL_RESULT.with(|r| *r.borrow_mut() = None);
        let mut msg = MSG::default();
        loop {
            if !IsWindow(hwnd).as_bool() {
                break;
            }
            let q = GetMessageW(&mut msg, None, 0, 0);
            if q.0 <= 0 {
                PostQuitMessage(0);
                break;
            }
            if TranslateAcceleratorW(a.hwnd, HACCEL::default(), &msg) == 0 {
                if IsDialogMessageW(hwnd, &msg).as_bool() {
                    continue;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    MODAL_RESULT.with(|r| r.borrow().clone()).flatten()
}

thread_local! {
    static MODAL_RESULT: std::cell::RefCell<Option<Option<u32>>> = const { std::cell::RefCell::new(None) };
}

unsafe extern "system" fn goto_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (w.0 & 0xFFFF) as usize;
            if id == 3003 || id == 1 {
                let edit = GetDlgItem(hwnd, 3001).unwrap_or_default();
                let len = SendMessageW(edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as usize;
                let mut buf = vec![0u16; len + 1];
                SendMessageW(edit, WM_GETTEXT, WPARAM(len + 1), LPARAM(buf.as_mut_ptr() as isize));
                let s = String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(len)]);
                let n: Option<u32> = s.trim().parse().ok();
                MODAL_RESULT.with(|r| *r.borrow_mut() = Some(n));
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            } else if id == 3004 || id == 2 {
                MODAL_RESULT.with(|r| *r.borrow_mut() = Some(None));
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        WM_CLOSE => {
            MODAL_RESULT.with(|r| *r.borrow_mut() = Some(None));
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

/// ChooseFontW (comdlg32) — Format > Police...
pub fn font_dialog() -> Option<windows::Win32::Graphics::Gdi::LOGFONTW> {
    let a = app();
    let mut lf = windows::Win32::Graphics::Gdi::LOGFONTW::default();
    // Seed with current settings.
    for (dst, c) in lf.lfFaceName.iter_mut().zip(a.settings.font_face.encode_utf16()) {
        *dst = c;
    }
    lf.lfHeight = a.settings.font_height;
    lf.lfWeight = a.settings.font_weight as i32;
    lf.lfItalic = a.settings.font_italic as u8;
    lf.lfUnderline = a.settings.font_underline as u8;
    let mut cf = CHOOSEFONTW::default();
    cf.lStructSize = std::mem::size_of::<CHOOSEFONTW>() as u32;
    cf.hwndOwner = a.hwnd;
    cf.hDC = windows::Win32::Graphics::Gdi::HDC::default();
    cf.lpLogFont = &mut lf as *mut _;
    cf.Flags = CF_SCREENFONTS | CF_INITTOLOGFONTSTRUCT | CF_EFFECTS;
    if unsafe { ChooseFontW(&mut cf) }.as_bool() {
        Some(lf)
    } else {
        None
    }
}

/// PageSetupDlgW (comdlg32) — Fichier > Mise en page...
pub fn page_setup_dialog() -> bool {
    let a = app();
    let mut ps = PAGESETUPDLGW::default();
    ps.lStructSize = std::mem::size_of::<PAGESETUPDLGW>() as u32;
    ps.hwndOwner = a.hwnd;
    unsafe { PageSetupDlgW(&mut ps).as_bool() }
}
