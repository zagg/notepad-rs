//! Entry point of the full editor clone (`notepad-rs.exe`).
//!
//! Command line parity with the original notepad.exe (argv parsed with
//! CommandLineToArgvW in wWinMain FUN_1400101d8):
//!   notepad-rs.exe [/P] [/O] [/G] [fichier ...]
//!     /P  -> print then exit
//!     /O  -> open read-only
//!     /G  -> (classic: no tabs) accepted and ignored
//!     /M  -> (classic: monospace font) accepted and ignored

#![windows_subsystem = "windows"]
#![allow(dead_code)] // helpers kept for full behavioral parity

mod app;
mod autosave;
mod dialogs;
mod document;
mod encoding;
mod find;
mod print;
mod richedit_status;
mod state;
mod status;
mod toolbar;

use windows::core::PWSTR;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Environment::GetCommandLineW;
use windows::Win32::UI::Shell::CommandLineToArgvW;

fn main() {
    // Capture panics to a log file (GUI subsystem has no console).
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let _ = std::fs::write(
            std::path::Path::new(&std::env::var("TEMP").unwrap_or_default()).join("nrs-panic.log"),
            format!("{info} @ {loc}"),
        );
    }));

    unsafe {
        // STA, like the stub's wWinMain (CoInitializeEx COINIT_APARTMENTTHREADED).
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let (files, readonly, print_only) = parse_command_line();

    // PathFileExistsW semantics: keep only files that exist; directories ignored.
    let files: Vec<std::path::PathBuf> = files
        .into_iter()
        .filter(|p| {
            let w = notepad_rs::wf::to_wide(&p.display().to_string());
            unsafe {
                windows::Win32::UI::Shell::PathFileExistsW(windows::core::PCWSTR(w.as_ptr()))
            }
            .is_ok()
        })
        .collect();

    let code = app::run(files, readonly, print_only);
    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }
    std::process::exit(code);
}

fn parse_command_line() -> (Vec<std::path::PathBuf>, bool, bool) {
    let mut files = Vec::new();
    let mut readonly = false;
    let mut print_only = false;
    unsafe {
        let mut argc: i32 = 0;
        let base: *mut PWSTR = CommandLineToArgvW(GetCommandLineW(), &mut argc);
        if base.is_null() {
            return (files, readonly, print_only);
        }
        // argv[0] = image path (skipped like the original's quote-aware skip).
        for i in 1..argc.max(0) as usize {
            let arg = (*base.add(i)).to_string().unwrap_or_default();
            let lower = arg.to_lowercase();
            match lower.as_str() {
                "/p" => print_only = true,
                "/o" => readonly = true,
                "/g" | "/m" => {} // accepted for compatibility
                _ => {
                    let trimmed = arg.trim_matches('"');
                    if !trimmed.is_empty() {
                        files.push(std::path::PathBuf::from(trimmed));
                    }
                }
            }
        }
        let _ = windows::Win32::Foundation::LocalFree(
            windows::Win32::Foundation::HLOCAL(base as *mut core::ffi::c_void),
        );
    }
    (files, readonly, print_only)
}
