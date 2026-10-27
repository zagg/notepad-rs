//! RichEdit (riched20.dll) constants and structs.
//! The Store Notepad package ships its own riched20.dll (observed in the package dir),
//! and the original Notepad is built on a RichEdit control.

use windows::Win32::Foundation::{HANDLE, RECT};
use windows::Win32::Graphics::Gdi::HDC;

/// WM_USER-based RichEdit messages (riched.h values).
pub const WM_USER: u32 = 0x0400;

// EDIT/RichEdit messages with the 0x00B0 base from winuser.h (RichEdit honours the
// EDIT-compatible set). Values verified against winuser.h: EM_GETSEL=WM_USER+0,
// EM_SETSEL=WM_USER+1, ..., EM_UNDO=WM_USER+23, EM_EMPTYUNDOBUFFER=WM_USER+29.
pub const EM_GETSEL: u32 = 0x00B0;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EM_REPLACESEL: u32 = 0x00C2;
pub const EM_GETLINECOUNT: u32 = 0x00BA;
pub const EM_LINEINDEX: u32 = 0x00BB;
pub const EM_LINEFROMCHAR: u32 = 0x00C9;
pub const EM_LINESCROLL: u32 = 0x00B6;
pub const EM_UNDO: u32 = 0x00C7;
pub const EM_REDO: u32 = WM_USER + 84;
pub const EM_CANUNDO: u32 = 0x00C6;
pub const EM_CANREDO: u32 = WM_USER + 85;
pub const EM_EMPTYUNDOBUFFER: u32 = 0x00CD;
pub const EM_SETREADONLY: u32 = 0x00CF;
pub const EM_GETMODIFY: u32 = 0x00B8;
pub const EM_SETMODIFY: u32 = 0x00B9;
/// RichEdit selection as CHARRANGE (full 32-bit positions): riched.h WM_USER+52/+55.
pub const EM_EXGETSEL: u32 = WM_USER + 52;
pub const EM_EXSETSEL: u32 = WM_USER + 55;
pub const EM_SETTARGETDEVICE: u32 = WM_USER + 71;
pub const EM_SETTEXTMODE: u32 = WM_USER + 89;
pub const EM_SETUNDOLIMIT: u32 = WM_USER + 82;
pub const EM_SETBKGNDCOLOR: u32 = WM_USER + 67;
pub const EM_SETEVENTMASK: u32 = WM_USER + 69;
pub const EM_SETZOOM: u32 = WM_USER + 220;
pub const EM_SCROLLCARET: u32 = WM_USER + 49;
pub const EM_SETLIMITTEXT: u32 = WM_USER + 200;
pub const EM_FORMATRANGE: u32 = WM_USER + 57;

/// EN_* notification codes (delivered through WM_COMMAND from the control).
/// Values verified against the Win32 metadata (RichEdit): EN_SELCHANGE=0x0702,
/// EN_CHANGE=0x0300.
pub const EN_SELCHANGE: u32 = 0x0702;
pub const EN_UPDATE: u32 = 0x0400;
pub const EN_CHANGE: u32 = 0x0300;

/// ENM_* event flags for EM_SETEVENTMASK (RichEdit values from the Win32
/// metadata: ENM_CHANGE=0x1, ENM_UPDATE=0x2, ENM_SELCHANGE=0x00080000).
pub const ENM_SELCHANGE: u32 = 0x0008_0000;
pub const ENM_CHANGE: u32 = 0x0000_0001;
pub const ENM_UPDATE: u32 = 0x0000_0002;

/// TM_PLAINTEXT for EM_SETTEXTMODE.
pub const TM_PLAINTEXT: usize = 1;

/// RichEdit window classes. "RICHEDIT50W" comes from msftedit.dll, "RichEdit20W"
/// from riched20.dll (the Store package shipped its own riched20.dll copy).
pub const RICHEDIT_CLASS: &str = "RICHEDIT50W";
pub const RICHEDIT_CLASS_ALT: &str = "RichEdit20W";
pub const RICHED20_DLL: &str = "riched20.dll";
pub const MSFTEDIT_DLL: &str = "msftedit.dll";

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CHARRANGE {
    pub cp_min: i32,
    pub cp_max: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FORMATRANGE {
    pub hdc: HDC,
    pub hdc_target: HDC,
    pub rc: RECT,
    pub rc_page: RECT,
    pub charrange: CHARRANGE,
}

/// Opaque handle wrapper for LoadLibraryW result (only kept to keep the DLL resident).
pub type ModuleHandle = HANDLE;
