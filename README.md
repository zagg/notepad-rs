# notepad-rs — Notepad, rebuilt in Rust 🦀

What if Windows Notepad fit in **640 KB** instead of an 18 MB Store package?
`notepad-rs` is a pixel-faithful, **100 % Rust** clone of the Windows 11 Notepad
(`Microsoft.WindowsNotepad 11.2607`), reverse-engineered from the shipped
artifacts and rebuilt from scratch — **no Store app is ever launched**.

One crate, two binaries (in `target\release\`):

| Binary | Role |
|---|---|
| `notepad.exe` | **Stub** — a faithful replica of `C:\Windows\notepad.exe`: it `CreateProcessW`s the **neighbouring `notepad-rs.exe`**, never the packaged app. |
| `notepad-rs.exe` | **The full editor** — menus, tabs, RichEdit, formatting toolbar *on the menu line*, Find/Replace/Go-to, 5-field status bar, zoom, encodings/EOL, autosave & session restore, `/P` `/O` `/G`. |

## Why it's fun

- ⚡ **~0.65 MB on disk, ~21 MB RAM** vs **18 MB / ~104 MB** for the Store original (see [benchmarks](#-original-vs-clone)).
- 🎯 The formatting toolbar lives **on the menu bar line** with Fluent-style
  owner-drawn items (H1▾, lists▾, **B**, *I*, ~~S~~, link, table▾, format |
  writing tools▾, share, account, ⚙) — icons from Segoe MDL2 Assets, right
  group anchored to the right edge, zero extra rows.
- 🧊 Fallback banner when the editor binary is missing (cream `BannerClass`,
  "Get Notepad" button) — the Microsoft Store opens **only** if you click it.
- 🔬 Built purely from reverse-engineering evidence (Ghidra decompiles +
  shipped strings), down to registry keys and ETW tracing.

## Build

```
cargo +stable-x86_64-pc-windows-gnu build --release
```

## Use

```
notepad.exe [file ...]                     # stub → spawns notepad-rs.exe (local)
notepad-rs.exe [/P] [/O] [/G] [file ...]
```

- `/P` print dialog, `/O` read-only, `/G` `/M` accepted for compatibility.
- Formatting toolbar: **Affichage → Barre d'outils de mise en forme**
  (Ctrl+Shift+T). Left buttons insert Markdown (bold/italic/strike/link/
  H1▾/lists▾/table▾/format); right side = writing tools, share (copies the
  file path), account and ⚙ menus.

## 📊 Original vs clone

Measured on the same machine (Windows 11 x64, empty document, 5 s after launch):

| Metric | Store original | notepad-rs | Win |
|---|---:|---:|---|
| Editor binary | 3.15 MB (`Notepad.exe`) + 3.86 MB `riched20.dll` + 6.12 MB `NotepadXamlUI.dll` … | **0.38 MB** (`notepad-rs.exe`) | **~8×** |
| Total on disk | **18 MB** package (258 files) | **0.64 MB** (stub 267 KB + editor) | **~28×** |
| Launcher | `System32\notepad.exe` 352 KB | `notepad.exe` 267 KB | ~1.3× |
| RAM (working set) | **104 MB** | **21 MB** | **~5×** |

## Fidelity (same contracts as the original)

- Window class `Notepad`, French menus (« Fichier/Édition/Format/Affichage/Aide »).
- Shared registry keys under `HKCU\Software\Microsoft\Notepad`:
  `iWindowPosX/Y/DX/DY`, `fSaveWindowPositions`, `IsWordWrap`,
  `StatusBarVisibility`, `fWindowsOnlyEOL`, `ShowStoreBanner`, `ZoomLevel`,
  `DefaultFonts\lf*`, `IsFormattingToolbarVisible`.
- Autosave at `%LOCALAPPDATA%\Microsoft\Notepad\Autosave\` with a
  `probe.autosave` sentinel (leftover tabs are restored after a crash).
- RichEdit via `msftedit.dll` (`RICHEDIT50W`), fallback `riched20.dll`
  (`RichEdit20W`) — the Store package ships its own `riched20.dll` too.
- ETW tracing + `HeapSetInformation(TerminationOnCorruption)`, like the
  original `wWinMain`.

## 🔬 RE evidence ledger

Analysed artifacts:
- `C:\Windows\notepad.exe` — v10.0.26100.9278, SHA-256
  `468ffe129c395abf6b21a09efdf261910a95fb98aa982ead73caa7b2b684577e`.
  Proven to be a plain **launcher** (no editing UI of its own) —
  `ev_e8cfde73…`, `ev_9e512ae4…`, `ev_af11de32…`.
- `WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_x64__8wekyb3d8bbwe\Notepad\Notepad.exe`
  — the modern editor, characterised via shipped strings (`StatusBar*`,
  `Autosave`, `BannerClass`, formatting toolbar) — `ev_6f5ecdc1…`.

Reference functions from the stub's Ghidra decompile:

| Address | Role reproduced |
|---|---|
| `FUN_1400101d8` | wWinMain: HeapSetInformation, CoInitializeEx STA, ETW, argv (`CommandLineToArgvW`), message loop |
| `FUN_140013154` | NpPreStart: `PathFileExistsW` check for the packaged editor + `CreateProcessW` (else fallback banner) |
| `FUN_14000d9f0` | elevation probe (`OpenProcessToken`/`GetTokenInformation`) |
| `FUN_140023c00` | `CreateProcessW("<path>" args)` launcher, zeroed STARTUPINFO |
| `FUN_14000a234` / `FUN_14000a630` | `BannerClass` creation + WndProc (RGB 255,255,225; buttons 0x101 “Get”, 0x102 dismiss, 0x103 icon) |
| `FUN_14000aac4` | icon click → `ShowStoreBanner=0` + DestroyWindow |
| `FUN_14000f9e0` | `EVENT_OBJECT_LOCATIONCHANGE` hook → remembers banner position |

## Tests

Cross-process harness (`WM_*` Post/SendMessage) — all green:

| Suite | Covers | Result |
|---|---|---|
| `func.ps1` | UTF-8 BOM / UTF-16 LE / ANSI / UTF-8-LF round-trips | 4/4 |
| `func3.ps1` | window, tabs (real click → TCN_SELCHANGE), find (sel=39..44), go-to, zoom, read-only | 12/12 |
| `tbtest3.ps1` | toolbar buttons via `WM_COMMAND`: bold/italic Markdown wrap, H1 popup | 4/4 |
| `tbtoggle.ps1` | Ctrl+Shift+T show/hide of the menu-line toolbar | 2/2 |
| `verify.ps1` | stub end-to-end without Store, fallback banner + dismiss, autosave after kill, `/P` | 10/10 |

Cross-process harness gotchas: `WM_GETTEXT`/`WM_SETTEXT`/`WM_PASTE`/
`WM_COMMAND`/`WM_LBUTTON*` are marshalled (reliable); `SB_GETTEXT` and
pointer-carrying `WM_NOTIFY`/`EM_EXSETSEL` are **not** (crash comctl32 / err=5)
→ simulate clicks for tabs, type via `WM_CHAR` so the *modified* flag is real.

Implementation details and root causes of fixed bugs: see `DEV-NOTES.md` (FR).
