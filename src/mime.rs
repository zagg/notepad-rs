//! urlmon!FindMimeFromData — the original stub uses it to sniff the MIME type of files
//! given on the command line (import "URLMON.DLL::FindMimeFromData" observed by REA).

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Com::{CoTaskMemFree, Urlmon::FindMimeFromData};

/// Returns the sniffed MIME type ("text/plain", "text/html", ...). None on failure.
pub fn sniff_mime(bytes: &[u8]) -> Option<String> {
    let mut out = PWSTR::null();
    let res = unsafe {
        FindMimeFromData(
            None,
            PCWSTR::null(),
            Some(bytes.as_ptr() as *const core::ffi::c_void),
            bytes.len().min(u32::MAX as usize) as u32,
            PCWSTR::null(),
            0,
            &mut out,
            0,
        )
    };
    if res.is_ok() && !out.is_null() {
        let s = unsafe { out.to_string().ok()? };
        unsafe {
            CoTaskMemFree(Some(out.0 as *const _));
        }
        Some(s)
    } else {
        None
    }
}
