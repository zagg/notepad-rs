//! Printing via PrintDlgW (comdlg32) + EM_FORMATRANGE (RichEdit), like Notepad:
//! the Store package bundles riched20.dll (observed) which provides EM_FORMATRANGE.

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DeleteDC, GetDeviceCaps, SetMapMode, HORZRES, LOGPIXELSX, LOGPIXELSY, MM_TEXT, VERTRES, HDC,
};
use windows::Win32::Storage::Xps::{AbortDoc, EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{PrintDlgW, PD_RETURNDC, PRINTDLGW};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

use notepad_rs::richedit::{CHARRANGE, EM_FORMATRANGE, FORMATRANGE};

/// Show the Print dialog and print the current control content.
pub fn print_with_dialog(parent: HWND) -> bool {
    unsafe {
        let mut pd = PRINTDLGW::default();
        pd.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
        pd.hwndOwner = parent;
        pd.hInstance = GetModuleHandleW(None).unwrap_or_default().into();
        pd.Flags = PD_RETURNDC;
        if !PrintDlgW(&mut pd).as_bool() {
            return false;
        }
        let hdc = pd.hDC;
        if hdc.is_invalid() {
            return false;
        }
        let edit = crate::state::app().edit;
        let ok = print_hdc(edit, hdc);
        let _ = DeleteDC(hdc);
        ok
    }
}

unsafe fn print_hdc(edit: HWND, hdc: HDC) -> bool {
    let phys_x = GetDeviceCaps(hdc, HORZRES);
    let phys_y = GetDeviceCaps(hdc, VERTRES);
    let dpi_x = GetDeviceCaps(hdc, LOGPIXELSX).max(1);
    let dpi_y = GetDeviceCaps(hdc, LOGPIXELSY).max(1);
    SetMapMode(hdc, MM_TEXT);

    // One-inch margins like classic Notepad defaults.
    let rc = RECT {
        left: dpi_x,
        top: dpi_y,
        right: phys_x - dpi_x,
        bottom: phys_y - dpi_y,
    };
    let rc_page = RECT { left: 0, top: 0, right: phys_x, bottom: phys_y };

    let mut fr = FORMATRANGE {
        hdc,
        hdc_target: hdc,
        rc,
        rc_page,
        charrange: CHARRANGE { cp_min: 0, cp_max: -1 },
    };

    let name: Vec<u16> = "Bloc-notes".encode_utf16().chain(std::iter::once(0)).collect();
    let di = DOCINFOW {
        cbSize: std::mem::size_of::<DOCINFOW>() as i32,
        lpszDocName: windows::core::PCWSTR(name.as_ptr()),
        ..Default::default()
    };
    if StartDocW(hdc, &di) <= 0 {
        return false;
    }
    let mut page_start: i32 = 0;
    loop {
        if StartPage(hdc) <= 0 {
            AbortDoc(hdc);
            return false;
        }
        fr.charrange = CHARRANGE { cp_min: page_start, cp_max: -1 };
        let last = SendMessageW(edit, EM_FORMATRANGE, WPARAM(1), LPARAM(&mut fr as *mut FORMATRANGE as isize)).0 as i32;
        if EndPage(hdc) <= 0 {
            AbortDoc(hdc);
            return false;
        }
        if last <= page_start {
            break;
        }
        page_start = last;
    }
    EndDoc(hdc);
    // Let RichEdit release print state.
    SendMessageW(edit, EM_FORMATRANGE, WPARAM(0), LPARAM(0));
    true
}
