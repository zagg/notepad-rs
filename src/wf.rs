//! Wide-string helpers.

use windows::core::{PWSTR, PCWSTR};
use windows::Win32::UI::WindowsAndMessaging::HMENU;

/// UTF-16 (with NUL) from a Rust str.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convenient PCWSTR from an owned wide string.
pub fn pcw(s: &[u16]) -> PCWSTR {
    PCWSTR(s.as_ptr())
}

/// Child-window control id packed into the HMENU slot of CreateWindowExW.
pub fn child_id(id: usize) -> HMENU {
    HMENU(id as *mut core::ffi::c_void)
}

/// The null menu handle (for windows that have no control id / menu).
pub fn no_menu() -> HMENU {
    HMENU::default()
}


/// Decode UTF-16 up to the first NUL.
pub fn from_wide(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

/// Buffer + ptr pair usable as an output PWSTR.
pub struct WideBuf {
    pub buf: Vec<u16>,
}

impl WideBuf {
    pub fn new(len: usize) -> Self {
        Self { buf: vec![0u16; len] }
    }
    pub fn as_mut_ptr(&mut self) -> PWSTR {
        PWSTR(self.buf.as_mut_ptr())
    }
    pub fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.buf.as_ptr())
    }
    pub fn to_string(&self) -> String {
        from_wide(&self.buf)
    }
}
