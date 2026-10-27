//! Main window: class "Notepad" (same as the original), menu bar, tab strip,
//! RichEdit control, status bar, accelerators, drag & drop, zoom, autosave timer.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, GetDC, GetSysColorBrush, ReleaseDC, COLOR_WINDOW, DEFAULT_CHARSET,
    LOGFONTW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, NMHDR, SBARS_TOOLTIPS, SB_SETPARTS, TCS_FOCUSNEVER, TCS_TABS,
    ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX,
    DRAWITEMSTRUCT, MEASUREITEMSTRUCT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::state::app;
use notepad_rs::richedit::{
    EM_GETMODIFY, EM_REDO, EM_REPLACESEL, EM_SETTARGETDEVICE, EM_SETUNDOLIMIT, EM_SETZOOM,
    EM_UNDO, EN_CHANGE, EN_SELCHANGE, ENM_CHANGE, ENM_SELCHANGE, MSFTEDIT_DLL, RICHED20_DLL,
    RICHEDIT_CLASS, RICHEDIT_CLASS_ALT, EM_SETEVENTMASK,
};
use notepad_rs::wf::{child_id, pcw, to_wide};
use notepad_rs::FEEDBACK_URI;

pub mod cmd {
    pub const NEW: usize = 30001;
    pub const OPEN: usize = 30002;
    pub const SAVE: usize = 30003;
    pub const SAVE_AS: usize = 30004;
    pub const SAVE_ALL: usize = 30005;
    pub const PAGE_SETUP: usize = 30006;
    pub const PRINT: usize = 30007;
    pub const EXIT: usize = 30008;
    pub const UNDO: usize = 30011;
    pub const REDO: usize = 30012;
    pub const CUT: usize = 30013;
    pub const COPY: usize = 30014;
    pub const PASTE: usize = 30015;
    pub const DELETE: usize = 30016;
    pub const SELECT_ALL: usize = 30017;
    pub const TIME_DATE: usize = 30018;
    pub const FIND: usize = 30021;
    pub const FIND_NEXT: usize = 30022;
    pub const REPLACE: usize = 30023;
    pub const GOTO: usize = 30024;
    pub const WORD_WRAP: usize = 30031;
    pub const FONT: usize = 30032;
    pub const STATUS_BAR: usize = 30041;
    pub const TOOLBAR_TOGGLE: usize = 30042;
    pub const ZOOM_FOLDER: usize = 30051;
    pub const ZOOM_IN: usize = 30052;
    pub const ZOOM_OUT: usize = 30053;
    pub const ZOOM_100: usize = 30054;
    pub const FEEDBACK: usize = 30061;
    pub const ABOUT: usize = 30062;
    pub const NEW_TAB: usize = 30071;
    pub const CLOSE_TAB: usize = 30072;
    pub const EDIT_CTRL: usize = 3000;
    pub const TAB_CTRL: usize = 4000;
}

const AUTOSAVE_TIMER: usize = 2;

/// SysTabControl32 messages we send ourselves.
const TCM_FIRST: u32 = 0x1300;
const TCM_INSERTITEMW: u32 = TCM_FIRST + 62;
const TCM_DELETEALLITEMS: u32 = TCM_FIRST + 9;
const TCM_GETCURSEL: u32 = TCM_FIRST + 11;
const TCM_SETCURSEL: u32 = TCM_FIRST + 12;
const TCIF_TEXT: u32 = 0x0001;
const TCN_SELCHANGE: u32 = (-551i32) as u32;

#[repr(C)]
struct TCITEMW {
    mask: u32,
    dw_state: u32,
    dw_state_mask: u32,
    psz_text: *mut u16,
    cch_text_max: i32,
    i_image: i32,
    l_param: isize,
}

pub fn run(initial_files: Vec<std::path::PathBuf>, readonly: bool, print_on_start: bool) -> i32 {
    unsafe {
        let hinst: HINSTANCE = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(_) => return 1,
        };
        let _ = InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwICC: ICC_BAR_CLASSES | ICC_STANDARD_CLASSES | ICC_TAB_CLASSES,
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        });
        // Load the RichEdit libraries explicitly: msftedit.dll registers the
        // "RICHEDIT50W" class, riched20.dll registers "RichEdit20W" (the Store
        // package shipped its own riched20.dll copy).
        let _ = windows::Win32::System::LibraryLoader::LoadLibraryW(pcw(&to_wide(MSFTEDIT_DLL)));
        let _ = windows::Win32::System::LibraryLoader::LoadLibraryW(pcw(&to_wide(RICHED20_DLL)));

        // State.
        let settings = notepad_rs::settings::Settings::load();
        // Session restore only when no file was requested (original behavior:
        // a file on the command line always wins).
        let docs = if !initial_files.is_empty() {
            Vec::new()
        } else if crate::autosave::crash_happened() {
            let r = crate::autosave::restore();
            if r.is_empty() {
                vec![crate::document::Doc::new_untitled(if settings.windows_eol {
                    crate::encoding::Eol::CrLf
                } else {
                    crate::encoding::Eol::Lf
                })]
            } else {
                r
            }
        } else {
            Vec::new()
        };
        crate::autosave::write_probe();

        let mut docs = docs;
        for f in &initial_files {
            let data = std::fs::read(f).ok();
            let (enc, text, eol) = data
                .map(|d| {
                    let eol = crate::encoding::detect_eol(&d);
                    let (e, t) = crate::encoding::decode(&d);
                    (e, t, eol)
                })
                .unwrap_or((
                    crate::encoding::Enc::Utf8,
                    String::new(),
                    crate::encoding::Eol::CrLf,
                ));
            docs.push(crate::document::Doc {
                path: Some(f.clone()),
                text,
                eol,
                enc,
                modified: false,
                readonly,
            });
        }
        if docs.is_empty() {
            docs.push(crate::document::Doc::new_untitled(if settings.windows_eol {
                crate::encoding::Eol::CrLf
            } else {
                crate::encoding::Eol::Lf
            }));
        }

        crate::state::install(Box::new(crate::state::App {
            hwnd: HWND::default(),
            edit: HWND::default(),
            status: HWND::default(),
            tab: HWND::default(),
            toolbar_visible: settings.toolbar_visible,
            docs,
            cur: 0,
            zoom: settings.zoom as i32,
            wrap: settings.word_wrap,
            status_visible: settings.status_bar,
            settings,
            find: crate::state::FindState::default(),
            autosave_dir: crate::autosave::ensure_dir(),
            in_dialog: false,
            exit_after_print: print_on_start,
        }));

        let class = to_wide("Notepad");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: pcw(&class),
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            return 1;
        }

        let x = app().settings.win_x;
        let y = app().settings.win_y;
        let w = app().settings.win_w.max(400);
        let h = app().settings.win_h.max(300);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            pcw(&class),
            pcw(&to_wide("Sans titre - Bloc-notes")),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            if x == 0 && y == 0 { CW_USEDEFAULT } else { x },
            if x == 0 && y == 0 { CW_USEDEFAULT } else { y },
            if x == 0 && y == 0 { CW_USEDEFAULT } else { w },
            if x == 0 && y == 0 { CW_USEDEFAULT } else { h },
            HWND::default(),
            build_menu(),
            hinst,
            None,
        )
        .unwrap_or_default();
        if hwnd.is_invalid() {
            return 1;
        }
        DragAcceptFiles(hwnd, true);
        app().hwnd = hwnd;
        // Espaceur de la barre d'outils à la largeur réelle de la fenêtre cliente.
        crate::toolbar::refresh_menu();
        crate::document::show_current();
        crate::status::update();
        if print_on_start {
            // /P: behave like the original — open the print dialog immediately.
            let _ = PostMessageW(hwnd, WM_COMMAND, WPARAM(cmd::PRINT), LPARAM(0));
        }

        let accels = accelerators();
        let haccel = CreateAcceleratorTableW(&accels).unwrap_or_default();

        let mut msg = MSG::default();
        loop {
            let r = GetMessageW(&mut msg, None, 0, 0);
            if r.0 <= 0 {
                break;
            }
            if !haccel.is_invalid() && TranslateAcceleratorW(hwnd, haccel, &msg) != 0 {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        msg.wParam.0 as i32
    }
}

fn accelerators() -> Vec<ACCEL> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VIRTUAL_KEY, VK_0, VK_A, VK_C, VK_DELETE, VK_ESCAPE, VK_F3, VK_F5, VK_F12, VK_G,
        VK_N, VK_O, VK_OEM_MINUS, VK_OEM_PLUS, VK_P, VK_S, VK_T, VK_V, VK_W, VK_X, VK_Y, VK_Z,
    };
    let f = |virt: ACCEL_VIRT_FLAGS, key: VIRTUAL_KEY, cmd: usize| ACCEL {
        fVirt: virt,
        key: key.0,
        cmd: cmd as u16,
    };
    let none = ACCEL_VIRT_FLAGS(0);
    vec![
        f(FCONTROL, VK_N, cmd::NEW),
        f(FCONTROL, VK_O, cmd::OPEN),
        f(FCONTROL, VK_S, cmd::SAVE),
        f(none, VK_F12, cmd::SAVE_AS),
        f(FCONTROL, VK_P, cmd::PRINT),
        f(none, VK_ESCAPE, cmd::EXIT),
        f(FCONTROL, VK_Z, cmd::UNDO),
        f(FCONTROL, VK_Y, cmd::REDO),
        f(FCONTROL, VK_X, cmd::CUT),
        f(FCONTROL, VK_C, cmd::COPY),
        f(FCONTROL, VK_V, cmd::PASTE),
        f(none, VK_DELETE, cmd::DELETE),
        f(FCONTROL, VK_A, cmd::SELECT_ALL),
        f(none, VK_F5, cmd::TIME_DATE),
        f(FCONTROL, VIRTUAL_KEY(0x46), cmd::FIND),
        f(none, VK_F3, cmd::FIND_NEXT),
        f(FSHIFT, VK_F3, cmd::FIND_NEXT | 0x4000),
        f(FCONTROL, VIRTUAL_KEY(0x48), cmd::REPLACE),
        f(FCONTROL, VK_G, cmd::GOTO),
        f(FCONTROL, VK_T, cmd::NEW_TAB),
        f(FCONTROL | FSHIFT, VK_T, cmd::TOOLBAR_TOGGLE),
        f(FCONTROL, VK_W, cmd::CLOSE_TAB),
        f(FCONTROL, VK_OEM_PLUS, cmd::ZOOM_IN),
        f(FCONTROL, VIRTUAL_KEY(0xBB), cmd::ZOOM_IN),
        f(FCONTROL, VK_OEM_MINUS, cmd::ZOOM_OUT),
        f(FCONTROL, VIRTUAL_KEY(0xBD), cmd::ZOOM_OUT),
        f(FCONTROL, VK_0, cmd::ZOOM_100),
    ]
}

fn build_menu() -> HMENU {
    unsafe {
        let menu = CreateMenu().unwrap_or_default();
        let add = |parent: HMENU, label: &str, id: usize| {
            AppendMenuW(parent, MF_STRING, id, pcw(&to_wide(label))).ok();
        };

        let file = CreatePopupMenu().unwrap_or_default();
        add(file, "&Nouveau\tCtrl+N", cmd::NEW);
        add(file, "Nouvel onglet\tCtrl+T", cmd::NEW_TAB);
        add(file, "&Ouvrir...\tCtrl+O", cmd::OPEN);
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(file, "&Enregistrer\tCtrl+S", cmd::SAVE);
        add(file, "Enregistrer &sous...\tF12", cmd::SAVE_AS);
        add(file, "Enregistrer &tout", cmd::SAVE_ALL);
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(file, "&Mise en page...", cmd::PAGE_SETUP);
        add(file, "&Imprimer...\tCtrl+P", cmd::PRINT);
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(file, "&Quitter\tCtrl+F4", cmd::EXIT);
        AppendMenuW(menu, MF_POPUP, file.0 as usize, pcw(&to_wide("&Fichier"))).ok();

        let edit = CreatePopupMenu().unwrap_or_default();
        add(edit, "&Annuler\tCtrl+Z", cmd::UNDO);
        add(edit, "&Rétablir\tCtrl+Y", cmd::REDO);
        AppendMenuW(edit, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(edit, "Co&uper\tCtrl+X", cmd::CUT);
        add(edit, "&Copier\tCtrl+C", cmd::COPY);
        add(edit, "C&oller\tCtrl+V", cmd::PASTE);
        add(edit, "&Supprimer\tSuppr", cmd::DELETE);
        AppendMenuW(edit, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(edit, "Sé&lectionner tout\tCtrl+A", cmd::SELECT_ALL);
        add(edit, "Heure/&date\tF5", cmd::TIME_DATE);
        AppendMenuW(edit, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(edit, "&Rechercher...\tCtrl+F", cmd::FIND);
        add(edit, "Rechercher &suivant\tF3", cmd::FIND_NEXT);
        add(edit, "&Remplacer...\tCtrl+H", cmd::REPLACE);
        add(edit, "A&tteindre...\tCtrl+G", cmd::GOTO);
        AppendMenuW(menu, MF_POPUP, edit.0 as usize, pcw(&to_wide("&Édition"))).ok();

        let format = CreatePopupMenu().unwrap_or_default();
        add(format, "&Word wrap", cmd::WORD_WRAP);
        AppendMenuW(format, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(format, "&Police...", cmd::FONT);
        AppendMenuW(menu, MF_POPUP, format.0 as usize, pcw(&to_wide("&Format"))).ok();

        let view = CreatePopupMenu().unwrap_or_default();
        add(view, "&Barre d'état", cmd::STATUS_BAR);
        add(view, "Barre d'outils de &mise en forme\tCtrl+Shift+T", cmd::TOOLBAR_TOGGLE);
        AppendMenuW(view, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        let zoom = CreatePopupMenu().unwrap_or_default();
        add(zoom, "&Sous-dossier", cmd::ZOOM_FOLDER);
        AppendMenuW(zoom, MF_SEPARATOR, 0, PCWSTR::null()).ok();
        add(zoom, "&Agrandir\tCtrl++", cmd::ZOOM_IN);
        add(zoom, "&Réduire\tCtrl+-", cmd::ZOOM_OUT);
        add(zoom, "Sélectionner &100 %\tCtrl+0", cmd::ZOOM_100);
        AppendMenuW(view, MF_POPUP, zoom.0 as usize, pcw(&to_wide("&Zoom"))).ok();
        AppendMenuW(menu, MF_POPUP, view.0 as usize, pcw(&to_wide("&Affichage"))).ok();

        let help = CreatePopupMenu().unwrap_or_default();
        add(help, "&Envoyer des commentaires", cmd::FEEDBACK);
        add(help, "&À propos du Bloc-notes", cmd::ABOUT);
        AppendMenuW(menu, MF_POPUP, help.0 as usize, pcw(&to_wide("&Aide"))).ok();

        // Barre d'outils de mise en forme SUR la ligne des menus : éléments
        // owner-draw ajoutés après « Aide », avec espaceur pour le groupe droit
        // (comme l'original). Voir toolbar.rs (WM_MEASUREITEM/WM_DRAWITEM).
        if app().toolbar_visible {
            crate::toolbar::append_items(menu);
        }

        menu
    }
}

/// Reconstruit la barre de menus par-dessus l'existante (WM_SIZE, bascule de la
/// barre d'outils) : force un nouveau WM_MEASUREITEM, donc un espaceur recalculé.
pub fn rebuild_menus() {
    unsafe {
        let a = app();
        if a.hwnd.is_invalid() {
            return;
        }
        let menu = build_menu();
        SetMenu(a.hwnd, menu).ok();
        let _ = DrawMenuBar(a.hwnd);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
    match msg {
        WM_CREATE => {
            // Must be set before any layout(): WM_CREATE and the initial WM_SIZE
            // are dispatched *inside* CreateWindowExW, before the caller stores hwnd.
            app().hwnd = hwnd;
            let tab_class = to_wide("SysTabControl32");
            let tab = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                pcw(&tab_class),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(TCS_TABS | TCS_FOCUSNEVER),
                0, 0, 0, 0,
                hwnd,
                child_id(cmd::TAB_CTRL),
                hinst,
                None,
            )
            .unwrap_or_default();
            app().tab = tab;

            // RichEdit control: try msftedit's RICHEDIT50W, fall back to riched20's RichEdit20W.
            let exstyle = WINDOW_EX_STYLE(0x0000_0200 /* WS_EX_CLIENTEDGE */);
            let style = WS_CHILD
                | WS_VISIBLE
                | WS_VSCROLL
                | WS_HSCROLL
                | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL | ES_NOHIDESEL) as u32);
            let edit_class = to_wide(RICHEDIT_CLASS);
            let mut edit = CreateWindowExW(
                exstyle,
                pcw(&edit_class),
                PCWSTR::null(),
                style,
                0, 0, 0, 0,
                hwnd,
                child_id(cmd::EDIT_CTRL),
                hinst,
                None,
            )
            .unwrap_or_default();
            if edit.is_invalid() {
                let alt = to_wide(RICHEDIT_CLASS_ALT);
                edit = CreateWindowExW(
                    exstyle,
                    pcw(&alt),
                    PCWSTR::null(),
                    style,
                    0, 0, 0, 0,
                    hwnd,
                    child_id(cmd::EDIT_CTRL),
                    hinst,
                    None,
                )
                .unwrap_or_default();
            }
            app().edit = edit;
            SendMessageW(edit, EM_SETUNDOLIMIT, WPARAM(0x10000), LPARAM(0));
            SendMessageW(
                edit,
                EM_SETEVENTMASK,
                WPARAM(0),
                LPARAM((ENM_CHANGE | ENM_SELCHANGE) as isize),
            );

            let status_class = to_wide("msctls_statusbar32");
            let status = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                pcw(&status_class),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SBARS_TOOLTIPS),
                0, 0, 0, 0,
                hwnd,
                child_id(0),
                hinst,
                None,
            )
            .unwrap_or_default();
            app().status = status;

            apply_font();
            apply_wrap();
            apply_zoom();
            if !app().status_visible {
                let _ = ShowWindow(status, SW_HIDE);
            }
            layout();
            refresh_tabs();
            SetTimer(hwnd, AUTOSAVE_TIMER, 2000, None);
            LRESULT(0)
        }
        WM_SIZE => {
            layout();
            crate::status::update();
            // Largeur d'espaceur de la barre d'outils dépendante de la taille client.
            crate::toolbar::refresh_menu();
            LRESULT(0)
        }
        WM_MEASUREITEM => {
            if crate::toolbar::measure_item(l.0 as *mut MEASUREITEMSTRUCT) {
                LRESULT(1)
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        WM_DRAWITEM => {
            if crate::toolbar::draw_item(l.0 as *const DRAWITEMSTRUCT) {
                LRESULT(1)
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        WM_SETFOCUS => {
            let _ = SetFocus(app().edit);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (w.0 & 0xFFFF) as usize;
            let code = (w.0 >> 16) as u32;
            // Child control notifications (RichEdit EN_*, tab TCN_*): never commands.
            if (id == cmd::EDIT_CTRL || id == cmd::TAB_CTRL) && code != 0 {
                if id == cmd::EDIT_CTRL && code == EN_SELCHANGE {
                    crate::status::update();
                } else if id == cmd::EDIT_CTRL && code == EN_CHANGE {
                    let m = SendMessageW(app().edit, EM_GETMODIFY, WPARAM(0), LPARAM(0)).0 != 0;
                    let a = app();
                    if let Some(d) = a.docs.get_mut(a.cur) {
                        if d.modified != m {
                            d.modified = m;
                            update_title();
                            refresh_tabs();
                        }
                    }
                }
                return LRESULT(0);
            }
            if handle_command(hwnd, id) {
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        WM_NOTIFY => {
            let nh = &*(l.0 as *const NMHDR);
            if nh.hwndFrom == app().tab && nh.code == TCN_SELCHANGE {
                let idx = SendMessageW(app().tab, TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                if idx >= 0 {
                    crate::document::switch_to(idx as usize);
                }
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let keys = (w.0 & 0xFFFF) as u16;
            let delta = ((w.0 >> 16) & 0xFFFF) as i16;
            if keys & 0x0008 /* MK_CONTROL */ != 0 {
                let step = if delta > 0 { 10 } else { -10 };
                zoom_by(step);
            } else {
                return DefWindowProcW(hwnd, msg, w, l);
            }
            LRESULT(0)
        }
        WM_INITMENU => {
            update_menu_checks();
            LRESULT(0)
        }
        WM_TIMER => {
            if w.0 == AUTOSAVE_TIMER {
                crate::document::sync_from_control();
                crate::autosave::save_all();
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            let hdrop = HDROP(l.0 as *mut _);
            let mut buf = [0u16; 32768];
            let n = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
            for i in 0..n {
                let cnt = DragQueryFileW(hdrop, i, Some(&mut buf));
                if cnt > 0 {
                    let f =
                        std::path::PathBuf::from(notepad_rs::wf::from_wide(&buf[..cnt as usize]));
                    crate::document::open_path(&f);
                }
            }
            DragFinish(hdrop);
            LRESULT(0)
        }
        WM_CLOSE => {
            crate::document::sync_from_control();
            // Ask for every modified document.
            loop {
                let i = {
                    let a = app();
                    match a.docs.iter().enumerate().find(|(_, d)| d.modified) {
                        Some((i, _)) => i,
                        None => break,
                    }
                };
                let name = app().docs[i].title();
                if !crate::dialogs::confirm_save(&name) {
                    return LRESULT(0); // Cancel aborts close
                }
                let a = app();
                a.docs[i].modified = false;
            }
            // Persist settings (registry parity) and close autosave session.
            let mut rc = RECT::default();
            if GetWindowRect(hwnd, &mut rc).is_ok() {
                app().settings.win_x = rc.left;
                app().settings.win_y = rc.top;
                app().settings.win_w = rc.right - rc.left;
                app().settings.win_h = rc.bottom - rc.top;
            }
            let (wrap, status_visible, zoom) = (app().wrap, app().status_visible, app().zoom);
            {
                let a = app();
                a.settings.word_wrap = wrap;
                a.settings.status_bar = status_visible;
                a.settings.zoom = zoom.max(10) as u32;
                a.settings.save();
            }
            crate::autosave::save_all();
            crate::autosave::clear();
            crate::autosave::remove_probe();
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

unsafe fn handle_command(hwnd: HWND, id: usize) -> bool {
    // Boutons de la barre d'outils de mise en forme (éléments owner-draw du menu).
    if (5000..=5013).contains(&id) {
        return crate::toolbar::handle_command(id);
    }
    let a = app();
    let edit = a.edit;
    match id {
        cmd::NEW => crate::document::new_tab(),
        cmd::NEW_TAB => crate::document::new_tab(),
        cmd::OPEN => {
            if let Some(p) = crate::dialogs::open_dialog() {
                crate::document::open_path(&p);
            }
        }
        cmd::SAVE => {
            crate::document::save_current(false);
        }
        cmd::SAVE_AS => {
            crate::document::save_current(true);
        }
        cmd::SAVE_ALL => {
            for i in 0..app().docs.len() {
                if app().docs[i].modified {
                    app().cur = i;
                    crate::document::save_current(app().docs[i].path.is_none());
                }
            }
        }
        cmd::PAGE_SETUP => {
            crate::dialogs::page_setup_dialog();
        }
        cmd::PRINT => {
            crate::document::sync_from_control();
            crate::print::print_with_dialog(hwnd);
            if app().exit_after_print {
                PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)).ok();
            }
        }
        cmd::EXIT => {
            PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)).ok();
        }
        cmd::UNDO => {
            SendMessageW(edit, EM_UNDO, WPARAM(0), LPARAM(0));
        }
        cmd::REDO => {
            SendMessageW(edit, EM_REDO, WPARAM(0), LPARAM(0));
        }
        cmd::CUT => {
            SendMessageW(edit, WM_CUT, WPARAM(0), LPARAM(0));
        }
        cmd::COPY => {
            SendMessageW(edit, WM_COPY, WPARAM(0), LPARAM(0));
        }
        cmd::PASTE => {
            SendMessageW(edit, WM_PASTE, WPARAM(0), LPARAM(0));
        }
        cmd::DELETE => {
            SendMessageW(edit, WM_CLEAR, WPARAM(0), LPARAM(0));
        }
        cmd::SELECT_ALL => {
            SendMessageW(edit, notepad_rs::richedit::EM_SETSEL, WPARAM(0), LPARAM(-1));
        }
        cmd::TIME_DATE => {
            let t = unsafe { GetLocalTime() };
            let s = format!(
                "{:02}:{:02} {}/{:02}/{}",
                t.wHour, t.wMinute, t.wDay, t.wMonth, t.wYear
            );
            let w: Vec<u16> = s
                .replace('\n', "\r")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                SendMessageW(edit, EM_REPLACESEL, WPARAM(1), LPARAM(w.as_ptr() as isize));
            }
        }
        cmd::FIND => crate::find::open_find(hwnd),
        cmd::FIND_NEXT => crate::find::find_next_key((id & 0x4000) != 0),
        cmd::REPLACE => crate::find::open_replace(hwnd),
        cmd::GOTO => {
            if let Some(line) = crate::dialogs::goto_dialog() {
                goto_line(line);
            }
        }
        cmd::WORD_WRAP => {
            let a = app();
            a.wrap = !a.wrap;
            apply_wrap();
            notepad_rs::reg::set_dword(notepad_rs::REG_KEY, "IsWordWrap", a.wrap as u32);
        }
        cmd::FONT => {
            if let Some(lf) = crate::dialogs::font_dialog() {
                let face = String::from_utf16_lossy(
                    &lf.lfFaceName
                        [..lf.lfFaceName.iter().position(|&c| c == 0).unwrap_or(0)],
                );
                let a = app();
                if !face.is_empty() {
                    a.settings.font_face = face;
                }
                a.settings.font_height = lf.lfHeight;
                a.settings.font_weight = lf.lfWeight as u32;
                a.settings.font_italic = lf.lfItalic != 0;
                a.settings.font_underline = lf.lfUnderline != 0;
                apply_font();
            }
        }
        cmd::STATUS_BAR => {
            crate::status::set_visible(!app().status_visible);
        }
        cmd::TOOLBAR_TOGGLE => {
            let v = !app().toolbar_visible;
            app().toolbar_visible = v;
            rebuild_menus();
            notepad_rs::reg::set_dword(notepad_rs::REG_KEY, "IsFormattingToolbarVisible", v as u32);
        }
        cmd::ZOOM_IN => zoom_by(10),
        cmd::ZOOM_OUT => zoom_by(-10),
        cmd::ZOOM_100 => {
            app().zoom = 100;
            apply_zoom();
        }
        cmd::ZOOM_FOLDER => {}
        cmd::FEEDBACK => {
            ShellExecuteW(
                hwnd,
                PCWSTR::null(),
                pcw(&to_wide(FEEDBACK_URI)),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
        cmd::ABOUT => crate::dialogs::about_box(),
        cmd::CLOSE_TAB => {
            let cur = app().cur;
            if !crate::document::close_tab(cur, true) {
                return true;
            }
        }
        _ => return false,
    }
    true
}

fn goto_line(line: u32) {
    let a = app();
    let target = (line.saturating_sub(1)) as usize;
    unsafe {
        let cp =
            SendMessageW(a.edit, notepad_rs::richedit::EM_LINEINDEX, WPARAM(target), LPARAM(0)).0
                as usize;
        SendMessageW(
            a.edit,
            notepad_rs::richedit::EM_SETSEL,
            WPARAM(cp),
            LPARAM(cp as isize),
        );
        SendMessageW(a.edit, notepad_rs::richedit::EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    }
    crate::status::update();
}

fn zoom_by(step: i32) {
    let a = app();
    a.zoom = (a.zoom + step).clamp(10, 400);
    apply_zoom();
}

pub fn apply_zoom() {
    let a = app();
    unsafe {
        SendMessageW(a.edit, EM_SETZOOM, WPARAM(a.zoom as usize), LPARAM(100));
    }
    crate::status::update();
}

pub fn apply_wrap() {
    let a = app();
    unsafe {
        if a.wrap {
            SendMessageW(a.edit, EM_SETTARGETDEVICE, WPARAM(0), LPARAM(0));
        } else {
            let dc = GetDC(a.edit);
            SendMessageW(a.edit, EM_SETTARGETDEVICE, WPARAM(dc.0 as usize), LPARAM(1));
            ReleaseDC(a.edit, dc);
        }
    }
}

fn apply_font() {
    let a = app();
    let mut lf: LOGFONTW = unsafe { std::mem::zeroed() };
    lf.lfHeight = if a.settings.font_height != 0 {
        a.settings.font_height
    } else {
        -15
    };
    lf.lfWeight = a.settings.font_weight as i32;
    lf.lfItalic = a.settings.font_italic as u8;
    lf.lfUnderline = a.settings.font_underline as u8;
    lf.lfCharSet = DEFAULT_CHARSET;
    let face = if a.settings.font_face.is_empty() {
        "Consolas".to_string()
    } else {
        a.settings.font_face.clone()
    };
    for (dst, c) in lf.lfFaceName.iter_mut().zip(face.encode_utf16()) {
        *dst = c;
    }
    unsafe {
        let font = CreateFontIndirectW(&lf);
        SendMessageW(app().edit, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }
}

pub fn update_title() {
    let a = app();
    let t = a.docs[a.cur].window_title();
    unsafe {
        SetWindowTextW(a.hwnd, pcw(&to_wide(&t))).ok();
    }
}

pub fn refresh_tabs() {
    let a = app();
    unsafe {
        SendMessageW(a.tab, TCM_DELETEALLITEMS, WPARAM(0), LPARAM(0));
        let titles: Vec<String> = a.docs.iter().map(|d| d.title()).collect();
        for (i, t) in titles.iter().enumerate() {
            let mut w = to_wide(t);
            let item = TCITEMW {
                mask: TCIF_TEXT,
                dw_state: 0,
                dw_state_mask: 0,
                psz_text: w.as_mut_ptr(),
                cch_text_max: 0,
                i_image: -1,
                l_param: 0,
            };
            SendMessageW(
                a.tab,
                TCM_INSERTITEMW,
                WPARAM(i),
                LPARAM(&item as *const TCITEMW as isize),
            );
        }
        SendMessageW(a.tab, TCM_SETCURSEL, WPARAM(a.cur), LPARAM(0));
    }
}

pub fn layout() {
    let a = app();
    unsafe {
        let mut rc = RECT::default();
        GetClientRect(a.hwnd, &mut rc).ok();
        let tab_h = 26i32;
        let status_h = if a.status_visible { 22 } else { 0 };
        SetWindowPos(a.tab, HWND::default(), 0, 0, rc.right, tab_h, SWP_NOZORDER).ok();
        SetWindowPos(
            a.status,
            HWND::default(),
            0,
            rc.bottom - status_h,
            rc.right,
            status_h.max(1),
            SWP_NOZORDER,
        )
        .ok();
        // Five parts: Ln/Col (flex), zoom, EOL, encoding, char count (right side).
        let w = rc.right;
        let mut parts = [w - 380, w - 300, w - 190, w - 110, -1];
        SendMessageW(
            a.status,
            SB_SETPARTS,
            WPARAM(parts.len()),
            LPARAM(parts.as_mut_ptr() as isize),
        );
        crate::status::update();
        SetWindowPos(
            a.edit,
            HWND::default(),
            0,
            tab_h,
            rc.right,
            (rc.bottom - status_h - tab_h).max(1),
            SWP_NOZORDER,
        )
        .ok();
        apply_wrap();
    }
}

fn update_menu_checks() {
    let a = app();
    unsafe {
        let menu = GetMenu(a.hwnd);
        let view = GetSubMenu(menu, 3);
        if a.status_visible {
            CheckMenuItem(view, cmd::STATUS_BAR as u32, (MF_CHECKED | MF_BYCOMMAND).0);
        } else {
            CheckMenuItem(view, cmd::STATUS_BAR as u32, (MF_UNCHECKED | MF_BYCOMMAND).0);
        }
        if a.toolbar_visible {
            CheckMenuItem(view, cmd::TOOLBAR_TOGGLE as u32, (MF_CHECKED | MF_BYCOMMAND).0);
        } else {
            CheckMenuItem(view, cmd::TOOLBAR_TOGGLE as u32, (MF_UNCHECKED | MF_BYCOMMAND).0);
        }
        let format = GetSubMenu(menu, 2);
        if a.wrap {
            CheckMenuItem(format, cmd::WORD_WRAP as u32, (MF_CHECKED | MF_BYCOMMAND).0);
        } else {
            CheckMenuItem(format, cmd::WORD_WRAP as u32, (MF_UNCHECKED | MF_BYCOMMAND).0);
        }
    }
}
