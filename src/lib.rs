//! Shared helpers for the two binaries (stub `notepad.exe` + editor `notepad-rs.exe`).
//!
//! Reverse-engineered from C:\Windows\notepad.exe 10.0.26100.9278 with REA:
//! - SHA-256 468ffe129c395abf6b21a09efdf261910a95fb98aa982ead73caa7b2b684577e (ev_e8cfde73...)
//! - No MENU/DIALOG/STRING/ACCEL resources -> launcher stub (ev_9e512ae4...)
//! - wWinMain: FUN_1400101d8 (ETW, HeapSetInformation, CoInitializeEx STA, arg parsing, msg loop)
//! - NpPreStart: FUN_140013154 (PathFileExistsW, CreateProcessW, RoGetActivationFactory, ...)
//! - Banner: FUN_14000a234 + WndProc FUN_14000a630 ("BannerClass", RGB(255,255,225), buttons 0x101/0x102/0x103)

pub mod wf;
pub mod reg;
pub mod richedit;
pub mod mime;
pub mod etw;
pub mod settings;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;

/// Registry key used by the original stub and the Store editor (observed strings).
pub const REG_KEY: &str = r"Software\Microsoft\Notepad";
pub const REG_KEY_AUTOSAVE: &str = r"Software\Microsoft\Notepad\Autosave";
pub const REG_KEY_FONTS: &str = r"Software\Microsoft\Notepad\DefaultFonts";

/// Store package identity observed in the stub strings.
pub const PACKAGE_PFN: &str = "Microsoft.WindowsNotepad_8wekyb3d8bbwe";
pub const STORE_URI: &str = "ms-windows-store://pdp/?PFN=Microsoft.WindowsNotepad_8wekyb3d8bbwe";
pub const FEEDBACK_URI: &str = "feedback-hub://?tabid=2&contextid=1010";

/// Editor binary launched by the stub (Rust clone instead of the Store app).
pub const EDITOR_EXE: &str = "notepad-rs.exe";

/// Convenient PCWSTR from an owned wide string.
pub fn pcw(s: &[u16]) -> PCWSTR {
    PCWSTR(s.as_ptr())
}

/// WM_* / IDC helpers re-exported to keep call sites short.
pub type WndProcFn = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;
