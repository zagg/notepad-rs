//! Thin registry helpers over HKCU\Software\Microsoft\Notepad
//! (keys observed verbatim in the stub: iWindowPosX/Y/DX/DY, fSaveWindowPositions,
//!  fWindowsOnlyEOL, IsWordWrap, StatusBarVisibility, ShowStoreBanner, DefaultFonts, Autosave).

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCreateKeyExW, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE, RRF_RT_REG_DWORD,
    RRF_RT_REG_SZ,
};

use crate::wf::to_wide;

fn hkcu(key: &str) -> (HKEY, Vec<u16>) {
    (HKEY_CURRENT_USER, to_wide(key))
}

/// Read a DWORD value; returns `default` on any failure.
pub fn get_dword(key: &str, name: &str, default: u32) -> u32 {
    let (hkey, k) = hkcu(key);
    let n = to_wide(name);
    let mut val: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let mut vtype = REG_VALUE_TYPE::default();
    let res = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_DWORD,
            Some(&mut vtype as *mut REG_VALUE_TYPE as *mut _),
            Some(&mut val as *mut u32 as *mut _),
            Some(&mut size as *mut u32 as *mut _),
        )
    };
    if res.is_ok() {
        val
    } else {
        default
    }
}

/// Read a SZ value; returns `default` on any failure.
pub fn get_sz(key: &str, name: &str, default: &str) -> String {
    let (hkey, k) = hkcu(key);
    let n = to_wide(name);
    let mut buf = vec![0u16; 512];
    let mut size = (buf.len() * 2) as u32;
    let mut vtype = REG_VALUE_TYPE::default();
    let res = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            Some(&mut vtype as *mut REG_VALUE_TYPE as *mut _),
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size as *mut u32 as *mut _),
        )
    };
    if res.is_ok() {
        crate::wf::from_wide(&buf)
    } else {
        default.to_string()
    }
}

pub fn set_dword(key: &str, name: &str, value: u32) {
    let (hkey, k) = hkcu(key);
    let n = to_wide(name);
    unsafe {
        let _ = RegSetKeyValueW(
            hkey,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            REG_DWORD.0,
            Some(&value as *const u32 as *const core::ffi::c_void),
            4,
        );
    }
}

pub fn set_sz(key: &str, name: &str, value: &str) {
    let (hkey, k) = hkcu(key);
    let n = to_wide(name);
    let v = to_wide(value);
    unsafe {
        let _ = RegSetKeyValueW(
            hkey,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            REG_SZ.0,
            Some(v.as_ptr() as *const core::ffi::c_void),
            (v.len() * 2) as u32,
        );
    }
}

pub fn delete_value(key: &str, name: &str) {
    let (hkey, k) = hkcu(key);
    let n = to_wide(name);
    unsafe {
        let _ = RegDeleteKeyValueW(hkey, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()));
    }
}

/// Ensure the key exists (mirrors RegCreateKeyExW usage in the original).
#[allow(dead_code)]
pub fn ensure_key(key: &str) {
    let (hkey, k) = hkcu(key);
    let mut h = HKEY::default();
    unsafe {
        let _ = RegCreateKeyExW(
            hkey,
            PCWSTR(k.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut h,
            None,
        );
    }
}
