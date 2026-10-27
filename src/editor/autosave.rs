//! Autosave + session restore.
//! Observed strings: "Software\Microsoft\Notepad\Autosave", "%s\%s.autosave",
//! "probe.autosave", "AutosaveFiles" (stub & Store editor).

use std::path::{Path, PathBuf};

use crate::document::Doc;
use crate::encoding::{Enc, Eol};

fn local_appdata() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    Path::new(&base).join("Microsoft").join("Notepad").join("Autosave")
}

pub fn ensure_dir() -> PathBuf {
    let d = local_appdata();
    let _ = std::fs::create_dir_all(&d);
    d
}

/// The original writes a "probe.autosave" sentinel while running; its absence at
/// startup means a clean previous shutdown.
pub fn probe_path() -> PathBuf {
    ensure_dir().join("probe.autosave")
}

pub fn write_probe() {
    let _ = std::fs::write(probe_path(), b"");
}

pub fn remove_probe() {
    let _ = std::fs::remove_file(probe_path());
}

pub fn crash_happened() -> bool {
    probe_path().exists()
}

fn sanitize(p: &Option<PathBuf>, i: usize) -> String {
    match p {
        Some(p) => p
            .file_name()
            .map(|f| f.to_string_lossy().chars().map(|c| if c.is_alphanumeric() || c == '.' { c } else { '_' }).collect::<String>())
            .unwrap_or_else(|| format!("tab{i}")),
        None => format!("untitled{i}"),
    }
}

/// Persist all modified documents (called from the autosave timer).
pub fn save_all() {
    let a = crate::state::app();
    let dir = ensure_dir();
    for (i, d) in a.docs.iter().enumerate() {
        if !d.modified {
            continue;
        }
        let name = format!("{}.autosave", sanitize(&d.path, i));
        let header = format!(
            "AUTOSAVE-V1\n{}\t{}\t{}\t{}\n",
            d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
            match d.eol {
                Eol::CrLf => "CRLF",
                Eol::Lf => "LF",
                Eol::Cr => "CR",
            },
            match d.enc {
                Enc::Utf8 => "UTF8",
                Enc::Utf8Bom => "UTF8BOM",
                Enc::Utf16Le => "UTF16LE",
                Enc::Utf16Be => "UTF16BE",
                Enc::Ansi => "ANSI",
            },
            d.readonly as u8,
        );
        let content = format!("{header}{}", d.text);
        let _ = std::fs::write(dir.join(name), content);
    }
}

/// Read back every *.autosave (excluding the probe) after a crash.
pub fn restore() -> Vec<Doc> {
    let dir = ensure_dir();
    let mut docs = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return docs,
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|f| f.to_string_lossy().ends_with(".autosave") && f != "probe.autosave")
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    for f in files {
        let data = match std::fs::read_to_string(&f) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let mut lines = data.splitn(3, '\n');
        let magic = lines.next().unwrap_or("");
        if !magic.starts_with("AUTOSAVE-V1") {
            continue;
        }
        let meta = lines.next().unwrap_or("");
        let text = lines.next().unwrap_or("").to_string();
        let mut it = meta.split('\t');
        let path = it.next().unwrap_or("-");
        let eol = match it.next().unwrap_or("CRLF") {
            "LF" => Eol::Lf,
            "CR" => Eol::Cr,
            _ => Eol::CrLf,
        };
        let enc = match it.next().unwrap_or("UTF8") {
            "UTF8BOM" => Enc::Utf8Bom,
            "UTF16LE" => Enc::Utf16Le,
            "UTF16BE" => Enc::Utf16Be,
            "ANSI" => Enc::Ansi,
            _ => Enc::Utf8,
        };
        let readonly = it.next().unwrap_or("0") == "1";
        docs.push(Doc {
            path: if path == "-" { None } else { Some(PathBuf::from(path)) },
            text,
            eol,
            enc,
            modified: true,
            readonly,
        });
    }
    docs
}

pub fn clear() {
    let dir = ensure_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.file_name().map(|f| f.to_string_lossy().ends_with(".autosave")).unwrap_or(false) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}
