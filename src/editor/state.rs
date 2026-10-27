//! Global application state (single-threaded Win32 app).

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;

use crate::document::Doc;
use notepad_rs::settings::Settings;

pub struct FindState {
    pub text: String,
    pub replace: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub wrap_around: bool,
    pub forward: bool,
    pub hwnd_find: HWND,
    pub hwnd_replace: HWND,
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            text: String::new(),
            replace: String::new(),
            match_case: false,
            whole_word: false,
            wrap_around: true,
            forward: true,
            hwnd_find: HWND::default(),
            hwnd_replace: HWND::default(),
        }
    }
}

pub struct App {
    pub hwnd: HWND,
    pub edit: HWND,
    pub status: HWND,
    pub tab: HWND,
    pub toolbar_visible: bool,
    pub docs: Vec<Doc>,
    pub cur: usize,
    pub zoom: i32,
    pub wrap: bool,
    pub status_visible: bool,
    pub settings: Settings,
    pub find: FindState,
    pub autosave_dir: PathBuf,
    /// Set while a save/close prompt is active to avoid re-entrant dialogs.
    pub in_dialog: bool,
    /// /P switch: print once at startup, then close.
    pub exit_after_print: bool,
}

static mut APP: *mut App = std::ptr::null_mut();

pub fn install(app: Box<App>) {
    unsafe {
        APP = Box::into_raw(app);
    }
}

/// Single-threaded accessor. The app runs one message loop on one thread.
pub fn app() -> &'static mut App {
    unsafe { &mut *APP }
}

pub fn has_app() -> bool {
    unsafe { !APP.is_null() }
}
