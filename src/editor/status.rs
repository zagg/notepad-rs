//! Status bar: "Ln, Col", zoom %, EOL, encoding, character count.
//! Mirrors the modern editor keys StatusBarLineColumn, StatusBarZoomPercentage,
//! StatusBarLineEndings*, StatusBarCharacterCount (REA strings).

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::SB_SETTEXTW;
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, SW_HIDE, SW_SHOW};

use crate::richedit_status::em_line_from_char;
use crate::state::app;
use notepad_rs::richedit::{CHARRANGE, EM_EXGETSEL, EM_LINEINDEX};

pub fn update() {
    let a = app();
    if !a.status_visible {
        return;
    }
    let (start, _end) = unsafe {
        let mut cr = CHARRANGE::default();
        SendMessageW(a.edit, EM_EXGETSEL, WPARAM(0), LPARAM(&mut cr as *mut _ as isize));
        (cr.cp_min.max(0) as u32, cr.cp_max.max(0) as u32)
    };
    let line0 = em_line_from_char(a.edit, start as i32);
    let line_start =
        unsafe { SendMessageW(a.edit, EM_LINEINDEX, WPARAM(line0 as usize), LPARAM(0)).0 } as u32;
    let col = start.saturating_sub(line_start) + 1;

    let chars = unsafe {
        windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            a.edit,
            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
            WPARAM(0),
            LPARAM(0),
        )
        .0
    };
    let (eol_label, enc_label) = {
        let d = &a.docs[a.cur];
        (d.eol.label(), d.enc.label())
    };
    let parts = [
        format!("Ligne {}, Colonne {}", line0 + 1, col),
        format!("{} %", a.zoom),
        eol_label.to_string(),
        enc_label.to_string(),
        format!("{chars} caractères"),
    ];
    // SB_SETTEXT does NOT copy the string: comctl32 keeps the pointer and reads it
    // later (paint, SB_GETTEXT). Keep the buffers alive until the next update.
    PART_STRINGS.with(|c| {
        let mut keep = c.borrow_mut();
        keep.clear();
        for (i, p) in parts.iter().enumerate() {
            let w: Vec<u16> = p.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                SendMessageW(a.status, SB_SETTEXTW, WPARAM(i), LPARAM(w.as_ptr() as isize));
            }
            keep.push(w);
        }
    });
}

thread_local! {
    static PART_STRINGS: std::cell::RefCell<Vec<Vec<u16>>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn set_visible(visible: bool) {
    let a = app();
    a.status_visible = visible;
    unsafe {
        if visible {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(a.status, SW_SHOW);
        } else {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(a.status, SW_HIDE);
        }
    }
    crate::app::layout();
    notepad_rs::reg::set_dword(notepad_rs::REG_KEY, "StatusBarVisibility", visible as u32);
    if visible {
        update();
    }
}
