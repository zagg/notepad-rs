//! Thin wrappers over a few RichEdit messages that need typed return handling.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

use notepad_rs::richedit::EM_LINEFROMCHAR;

pub fn em_line_from_char(edit: HWND, char_index: i32) -> i32 {
    unsafe {
        SendMessageW(edit, EM_LINEFROMCHAR, WPARAM(char_index as usize), LPARAM(0)).0 as i32
    }
}
