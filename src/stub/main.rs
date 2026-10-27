// Rust clone of the C:\Windows\notepad.exe launcher stub (v10.0.26100.9278).
//
// Behavior reproduced from REA evidence (see crate docs in lib.rs):
//  wWinMain (FUN_1400101d8):
//   - HeapSetInformation(HeapEnableTerminationOnCorruption)
//   - CoInitializeEx(COINIT_APARTMENTTHREADED)
//   - ETW provider with per-launch random GUID, events "LaunchNotepadStart"/"LaunchNotepadComplete"
//   - command line parsing that skips the image token (quote aware)
//  NpPreStart (FUN_140013154): PathFileExistsW check + CreateProcessW of the packaged
//   notepad; RegisterApplicationRestart; elevation probe (TokenElevation).
//  Banner (FUN_14000a234 / WndProc FUN_14000a630): "BannerClass", background
//   RGB(255,255,225), DT_WORDBREAK|DT_EDITCONTROL text, buttons 0x101 (Store) and
//   0x103 (owner-drawn icon -> writes ShowStoreBanner=0, destroys banner).
//
// Difference requested by the user: instead of the Store package
// (Microsoft.WindowsNotepad_8wekyb3d8bbwe) this stub launches the Rust editor
// notepad-rs.exe located next to it. The Store is only opened on explicit banner click.

#![windows_subsystem = "windows"]

use notepad_rs::etw::Tracing;
use notepad_rs::reg;
use notepad_rs::wf::{child_id, pcw, to_wide};
use notepad_rs::{EDITOR_EXE, REG_KEY, STORE_URI};

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, GetStockObject,
    SelectObject, DEFAULT_GUI_FONT, DT_EDITCONTROL, DT_WORDBREAK, HBRUSH, PAINTSTRUCT,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Environment::GetCommandLineW;
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleW, LoadLibraryExW, LOAD_LIBRARY_AS_DATAFILE,
};
use windows::Win32::System::Memory::{HeapSetInformation, HeapEnableTerminationOnCorruption};
use windows::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcess, GetStartupInfoW, OpenProcessToken, PROCESS_CREATION_FLAGS,
    PROCESS_INFORMATION, STARTUPINFOW, STARTF_USESHOWWINDOW,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, InitCommonControls, TTTOOLINFOW, TTM_ADDTOOLW, TTF_IDISHWND, TTF_SUBCLASS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, PathFileExistsW, ShellExecuteW, HDROP,
};
use windows::Win32::UI::WindowsAndMessaging::*;

// Banner command ids taken from the decompiled WndProc (FUN_14000a630).
const IDC_BANNER_STORE: usize = 0x101; // -> ms-windows-store://pdp/?PFN=...
const IDC_BANNER_DISMISS: usize = 0x102; // -> dismiss, keep going (classic path)
const IDC_BANNER_HIDE: usize = 0x103; // -> ShowStoreBanner=0 + DestroyWindow

const BANNER_BG: u32 = 0x00E1FFFF; // RGB(255,255,225) observed in WM_PAINT

static EDITOR_PATH: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn main() {
    unsafe {
        // --- wWinMain prologue (FUN_1400101d8) ---------------------------------
        HeapSetInformation(None, HeapEnableTerminationOnCorruption, None, 0).ok();
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let trace = Tracing::start("Microsoft.Notepad");

        // Recovery restart registration (RegisterApplicationRestart observed).
        let full_cmd = GetCommandLineW().to_string().unwrap_or_default();
        let _ = windows::Win32::System::Recovery::RegisterApplicationRestart(
            PCWSTR::null(),
            windows::Win32::System::Recovery::REGISTER_APPLICATION_RESTART_FLAGS(0),
        );

        // Elevation probe (FUN_14000d9f0: TokenElevation) - logged, not blocking.
        let elevated = is_elevated();
        trace.write(&format!("LaunchNotepadStart elevated={elevated}"));

        // Parse the command line the way wWinMain does (skip image token).
        let args = rest_of_command_line(&full_cmd);

        // --- locate the Rust editor (PathFileExistsW on the same dir) ----------
        let editor = find_editor();
        match editor {
            Some(path) => {
                trace.write(&format!("LaunchNotepadComplete editor={path}"));
                let ok = launch(&path, &args, None);
                if ok {
                    CoUninitialize();
                    std::process::exit(0);
                }
                // launch failed -> fall through to the banner
            }
            None => {
                trace.write("LaunchNotepadComplete editor=missing");
            }
        }

        // --- banner flow (ShowStoreBanner gate + BannerClass window) -----------
        let show_banner = reg::get_dword(REG_KEY, "ShowStoreBanner", 1);
        if show_banner == 0 {
            CoUninitialize();
            std::process::exit(0);
        }

        run_banner(&args);
        CoUninitialize();
        std::process::exit(0);
    }
}

/// FUN_14000d9f0: OpenProcessToken(TokenQuery) + GetTokenInformation(TokenElevation).
fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
            let mut elev = TOKEN_ELEVATION::default();
            let mut len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elev as *mut TOKEN_ELEVATION as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            )
            .is_ok();
            let _ = CloseHandle(token);
            return ok && elev.TokenIsElevated != 0;
        }
        false
    }
}

/// Reproduce wWinMain's quote-aware skip of the leading image token.
fn rest_of_command_line(cmd: &str) -> String {
    let mut chars = cmd.char_indices().peekable();
    let mut quoted = false;
    let mut pos = 0usize;
    // skip image token
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            quoted = !quoted;
        }
        if !quoted && (c == ' ' || c == '\t') {
            pos = i;
            break;
        }
        pos = i + c.len_utf8();
    }
    let rest = &cmd[pos..];
    rest.trim_start_matches([' ', '\t']).to_string()
}

fn find_editor() -> Option<String> {
    let mut buf = [0u16; 32768];
    let n = unsafe { GetModuleFileNameW(None, &mut buf) };
    if n == 0 {
        return None;
    }
    let me = notepad_rs::wf::from_wide(&buf[..n as usize]);
    let dir = std::path::Path::new(&me)
        .parent()?
        .to_string_lossy()
        .to_string();
    let candidate = std::path::Path::new(&dir).join(EDITOR_EXE);
    let w = to_wide(candidate.to_string_lossy().as_ref());
    if unsafe { PathFileExistsW(PCWSTR(w.as_ptr())) }.is_ok() {
        let _ = EDITOR_PATH.lock().map(|mut g| *g = candidate.to_string_lossy().to_string());
        Some(candidate.to_string_lossy().to_string())
    } else {
        None
    }
}

/// FUN_140023c00: CreateProcessW("<path>" args) with a zeroed STARTUPINFO.
fn launch(path: &str, args: &str, verb: Option<&str>) -> bool {
    let _ = verb;
    let cmdline = if args.is_empty() {
        format!("\"{path}\"")
    } else {
        format!("\"{path}\" {args}")
    };
    let mut wcmd = to_wide(&cmdline);
    let mut si = STARTUPINFOW::default();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut si_parent = STARTUPINFOW::default();
    si_parent.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    unsafe {
        GetStartupInfoW(&mut si_parent);
        if (si_parent.dwFlags & STARTF_USESHOWWINDOW) == STARTF_USESHOWWINDOW {
            si.dwFlags = STARTF_USESHOWWINDOW;
            si.wShowWindow = si_parent.wShowWindow;
        }
    }
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            PWSTR(wcmd.as_mut_ptr()),
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            None,
            &si,
            &mut pi,
        )
        .is_ok()
    }
}

// ---------------------------------------------------------------------------
// Banner implementation (mirror of FUN_14000a234 + FUN_14000a630)
// ---------------------------------------------------------------------------

unsafe fn run_banner(args: &str) {
    let hinst: HINSTANCE = match GetModuleHandleW(None) {
        Ok(h) => h.into(),
        Err(_) => return,
    };
    LoadLibraryExW(
        PCWSTR(to_wide("comdlg32.dll").as_ptr()),
        None,
        LOAD_LIBRARY_AS_DATAFILE,
    )
    .ok();

    InitCommonControls();

    let class = to_wide("BannerClass");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_CLASSDC,
        lpfnWndProc: Some(banner_wndproc),
        hInstance: hinst.into(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: HBRUSH::default(),
        lpszClassName: PCWSTR(class.as_ptr()),
        ..Default::default()
    };
    if RegisterClassExW(&wc) == 0 {
        return;
    }

    // Message-only main window (DAT_1400361b8 in the original).
    let main_class = to_wide("Notepad");
    let wc2 = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(def_wndproc),
        hInstance: hinst.into(),
        lpszClassName: PCWSTR(main_class.as_ptr()),
        ..Default::default()
    };
    RegisterClassExW(&wc2);
    let hwnd_main = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(main_class.as_ptr()),
        PCWSTR::null(),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        HWND::default(),
        None,
        hinst,
        None,
    )
    .unwrap_or_default();

    // Restore banner position (iWindowPosX/Y + fSaveWindowPositions, banner.cpp behavior).
    let x = reg::get_dword(REG_KEY, "iWindowPosX", 100) as i32;
    let y = reg::get_dword(REG_KEY, "iWindowPosY", 100) as i32;

    let banner = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class.as_ptr()),
        PCWSTR::null(),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        x,
        y,
        560,
        148,
        HWND::default(),
        None,
        hinst,
        None,
    )
    .unwrap_or_default();
    if banner.is_invalid() {
        return;
    }
    BANNER_ARGS.with(|a| *a.borrow_mut() = args.to_string());

    // Static texts (the original loads MUI strings 0xc1c/0xc1d/0xc1e).
    let btn_get = to_wide("Obtenir le Bloc-notes");
    let mut tip = to_wide("Ne plus afficher cette bannière");

    // Buttons are direct children of the banner window.
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        pcw(&to_wide("Button")),
        PCWSTR(btn_get.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x1000 /* BS_MULTILINE */ | 0x8000 /* BS_NOTIFY */),
        372,
        14,
        170,
        56,
        banner,
        child_id(IDC_BANNER_STORE),
        hinst,
        None,
    )
    .ok();
    let hide_btn = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        pcw(&to_wide("Button")),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
        344,
        80,
        22,
        22,
        banner,
        child_id(IDC_BANNER_HIDE),
        hinst,
        None,
    )
    .unwrap_or_default();

    // Tooltip on the icon button (tooltips_class32 + TTM_ADDTOOL in the original).
    let tt = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        pcw(&to_wide("tooltips_class32")),
        PCWSTR::null(),
        WS_POPUP,
        -32768,
        -32768,
        -32768,
        -32768,
        banner,
        None,
        hinst,
        None,
    )
    .unwrap_or_default();
    if !tt.is_invalid() {
        let mut tti = TTTOOLINFOW::default();
        tti.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
        tti.uFlags = TTF_IDISHWND | TTF_SUBCLASS;
        tti.hwnd = banner;
        tti.uId = hide_btn.0 as usize;
        tti.lpszText = PWSTR(tip.as_mut_ptr());
        SendMessageW(tt, TTM_ADDTOOLW, WPARAM(0), LPARAM(&tti as *const TTTOOLINFOW as isize));
    }

    // Accelerators: ESC/ENTER dismiss the banner (id 0x102 = WM_COMMAND handler).
    let accels = [
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_ESCAPE.0,
            cmd: IDC_BANNER_DISMISS as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_RETURN.0,
            cmd: IDC_BANNER_DISMISS as u16,
        },
    ];
    let haccel = CreateAcceleratorTableW(&accels).unwrap_or_default();

    // EVENT_OBJECT_LOCATIONCHANGE hook to persist the banner position (banner.cpp).
    let hook = SetWinEventHook(
        0x800b,
        0x800b,
        None,
        Some(location_hook),
        std::process::id(),
        0,
        WINEVENT_SKIPOWNPROCESS | WINEVENT_SKIPOWNTHREAD,
    );

    DragAcceptFiles(banner, true);

    let mut msg = MSG::default();
    loop {
        let r = GetMessageW(&mut msg, None, 0, 0);
        if r.0 <= 0 {
            break;
        }
        if !haccel.is_invalid() && TranslateAcceleratorW(banner, haccel, &msg) != 0 {
            continue;
        }
        if IsDialogMessageW(banner, &msg).as_bool() {
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    if !hook.is_invalid() {
        let _ = UnhookWinEvent(hook);
    }
    let _ = hwnd_main;
}

thread_local! {
    static BANNER_ARGS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Thin extern "system" trampoline around the generic DefWindowProcW wrapper.
unsafe extern "system" fn def_wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, w, l)
}

/// FUN_14000f9e0 equivalent: store banner position when moved.
unsafe extern "system" fn location_hook(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    let mut rc = RECT::default();
    if GetWindowRect(hwnd, &mut rc).is_ok() {
        reg::set_dword(REG_KEY, "iWindowPosX", rc.left as u32);
        reg::set_dword(REG_KEY, "iWindowPosY", rc.top as u32);
    }
}

unsafe extern "system" fn banner_wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let brush = CreateSolidBrush(COLORREF(BANNER_BG));
            FillRect(hdc, &ps.rcPaint, brush);
            let _ = DeleteObject(brush);
            // Stock GUI font (the original resolves the MUI banner font through CreateFontIndirectW).
            let font = GetStockObject(DEFAULT_GUI_FONT);
            let prev = SelectObject(hdc, font);
            let mut text = to_wide(
                "Impossible de démarrer l'application Bloc-notes.\nTéléchargez-la ou réinstallez-la.",
            );
            let mut rect = RECT {
                left: 14,
                top: 14,
                right: 350,
                bottom: 108,
            };
            DrawTextW(hdc, &mut text, &mut rect, DT_WORDBREAK | DT_EDITCONTROL);
            SelectObject(hdc, prev);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DRAWITEM => {
            // 0x103 owner-drawn info icon (ImageList_Draw of an info icon in the original).
            let di = &*(l.0 as *const DRAWITEMSTRUCT);
            if di.CtlID as usize == IDC_BANNER_HIDE {
                let icon = LoadIconW(None, IDI_INFORMATION).unwrap_or_default();
                DrawIconEx(
                    di.hDC,
                    0,
                    0,
                    icon,
                    22,
                    22,
                    0,
                    None,
                    DI_NORMAL,
                )
                .ok();
            }
            LRESULT(1)
        }
        WM_COMMAND => {
            let id = (w.0 & 0xFFFF) as usize;
            match id {
                IDC_BANNER_STORE => {
                    let _ = ShellExecuteW(
                        HWND::default(),
                        PCWSTR::null(),
                        pcw(&to_wide(STORE_URI)),
                        None,
                        None,
                        SW_SHOWNORMAL,
                    );
                    LRESULT(0)
                }
                IDC_BANNER_DISMISS => {
                    // 0x102 path in FUN_14000a630: set flag, persist, PostQuitMessage(0).
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                IDC_BANNER_HIDE => {
                    // FUN_14000aac4: RegSetKeyValueW(ShowStoreBanner=0) + DestroyWindow.
                    reg::set_dword(REG_KEY, "ShowStoreBanner", 0);
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
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_DROPFILES => {
            let hdrop = HDROP(l.0 as *mut _);
            let mut buf = [0u16; 32768];
            let n = DragQueryFileW(hdrop, 0, Some(&mut buf));
            if n > 0 {
                let file = notepad_rs::wf::from_wide(&buf[..n as usize]);
                let args = BANNER_ARGS.with(|a| a.borrow().clone());
                let editor = find_editor();
                if let Some(e) = editor {
                    launch(&e, &format!("\"{file}\" {args}"), None);
                    PostQuitMessage(0);
                }
            }
            DragFinish(hdrop);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
