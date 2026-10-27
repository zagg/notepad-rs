//! Encoding / EOL handling.
//! The Store editor exposes StatusBarLineEndings{CRLF,LF,CR}, FileTypeUTF8,
//! FileTypeUTF8WithBOM and EncodingComboBoxLabel (REA strings); the stub used
//! urlmon!FindMimeFromData for sniffing — reproduced in lib::mime.

use windows::Win32::Globalization::{
    MultiByteToWideChar, WideCharToMultiByte, CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Enc {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Ansi,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    CrLf,
    Lf,
    Cr,
}

impl Enc {
    /// Status-bar labels (modern Notepad style).
    pub fn label(&self) -> &'static str {
        match self {
            Enc::Utf8 => "UTF-8",
            Enc::Utf8Bom => "UTF-8 (BOM)",
            Enc::Utf16Le => "UTF-16 LE",
            Enc::Utf16Be => "UTF-16 BE",
            Enc::Ansi => "ANSI",
        }
    }
}

impl Eol {
    pub fn label(&self) -> &'static str {
        match self {
            Eol::CrLf => "Windows (CRLF)",
            Eol::Lf => "Unix (LF)",
            Eol::Cr => "Mac (CR)",
        }
    }
    pub fn marker(&self) -> &'static str {
        match self {
            Eol::CrLf => "\r\n",
            Eol::Lf => "\n",
            Eol::Cr => "\r",
        }
    }
}

/// Detect encoding from raw bytes and decode to an LF-normalized string.
pub fn decode(bytes: &[u8]) -> (Enc, String) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let s = String::from_utf8_lossy(&bytes[3..]).into_owned();
        return (Enc::Utf8Bom, normalize(&s));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let w: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return (Enc::Utf16Le, normalize(&String::from_utf16_lossy(&w)));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let w: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return (Enc::Utf16Be, normalize(&String::from_utf16_lossy(&w)));
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (Enc::Utf8, normalize(s)),
        Err(_) => {
            // ANSI fallback through the system code page.
            let text = ansi_to_utf16(bytes);
            (Enc::Ansi, normalize(&text))
        }
    }
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Encode an LF-normalized string to bytes for the given encoding, using `eol` line endings.
pub fn encode(text: &str, enc: Enc, eol: Eol) -> Vec<u8> {
    let text = match eol {
        Eol::Lf => text.to_string(),
        Eol::CrLf => text.replace('\n', "\r\n"),
        Eol::Cr => text.replace('\n', "\r"),
    };
    match enc {
        Enc::Utf8 => text.as_bytes().to_vec(),
        Enc::Utf8Bom => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(text.as_bytes());
            v
        }
        Enc::Utf16Le => {
            let mut v = vec![0xFF, 0xFE];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        Enc::Utf16Be => {
            let mut v = vec![0xFE, 0xFF];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            v
        }
        Enc::Ansi => utf16_to_ansi(&text),
    }
}

pub fn ansi_to_utf16(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let n = MultiByteToWideChar(
            CP_ACP,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            None,
        );
        if n <= 0 {
            return String::new();
        }
        let mut w = vec![0u16; n as usize];
        MultiByteToWideChar(
            CP_ACP,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            Some(&mut w),
        );
        String::from_utf16_lossy(&w)
    }
}

pub fn utf16_to_ansi(text: &str) -> Vec<u8> {
    let w: Vec<u16> = text.encode_utf16().collect();
    if w.is_empty() {
        return Vec::new();
    }
    unsafe {
        let n = WideCharToMultiByte(
            CP_ACP,
            0,
            &w,
            None,
            None,
            None,
        );
        if n <= 0 {
            return text.as_bytes().to_vec();
        }
        let mut out = vec![0u8; n as usize];
        WideCharToMultiByte(
            CP_ACP,
            0,
            &w,
            Some(&mut out),
            None,
            None,
        );
        out
    }
}

/// Guess the dominant EOL of an LF-normalized text by looking at the raw bytes.
pub fn detect_eol(bytes: &[u8]) -> Eol {
    let mut crlf = 0u32;
    let mut lf = 0u32;
    let mut cr = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    crlf += 1;
                    i += 2;
                    continue;
                }
                cr += 1;
            }
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }
    if crlf == 0 && lf == 0 && cr == 0 {
        Eol::CrLf
    } else if cr > lf && cr > crlf {
        Eol::Cr
    } else if lf > crlf {
        Eol::Lf
    } else {
        Eol::CrLf
    }
}
