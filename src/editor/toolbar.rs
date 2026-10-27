//! Barre d'outils de mise en forme INTÉGRÉE à la ligne de menus, comme l'original
//! (Bloc-notes Store 11.2607) : les boutons sont des éléments MF_OWNERDRAW ajoutés
//! après « Aide » dans la barre de menus — pas de ligne dédiée.
//!   groupe gauche : H1⌄  ☰⌄  B  I  S̶  lien  ⊞⌄  A(font)
//!   groupe droit  : ✎⌄   ↪   👤   ⚙   (ancrés à droite par un élément « espaceur »)
//! Les icônes viennent de « Segoe MDL2 Assets » (police système Win10/11), les
//! lettres B/I/S sont rendues en gras/italique/barré comme l'original.
//! Les boutons insèrent de la syntaxe Markdown dans le contrôle, comme le fait le
//! mode édition de l'original (le document stocke du Markdown en clair).

use std::cell::Cell;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DrawTextW, FillRect, GetDC, GetSysColorBrush, GetTextExtentPoint32W,
    ReleaseDC, SelectObject, COLOR_BTNFACE, COLOR_MENU, COLOR_MENUHILIGHT, DEFAULT_CHARSET,
    DT_LEFT, DT_SINGLELINE, DT_VCENTER, HFONT, HDC, LOGFONTW,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_SELECTED, ODT_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::cmd;
use crate::state::app;
use notepad_rs::richedit::{CHARRANGE, EM_EXGETSEL, EM_EXSETSEL, EM_LINEINDEX, EM_REPLACESEL};
use notepad_rs::wf::{pcw, to_wide};

/// Commandes internes des éléments owner-draw de la barre de menus
/// (aucun conflit avec cmd::*).
const TB_SPACER: usize = 5000;
const TB_H1: usize = 5001;
const TB_LIST: usize = 5002;
const TB_BOLD: usize = 5003;
const TB_ITALIC: usize = 5004;
const TB_STRIKE: usize = 5005;
const TB_LINK: usize = 5006;
const TB_TABLE: usize = 5007;
const TB_FORMAT: usize = 5008;
const TB_WRITE: usize = 5010;
const TB_SHARE: usize = 5011;
const TB_ACCOUNT: usize = 5012;
const TB_SETTINGS: usize = 5013;

/// Polices de rendu des éléments.
#[derive(Clone, Copy)]
enum F {
    Text,   // Segoe UI normal (« H1 »)
    Bold,   // Segoe UI gras   (« B »)
    Italic, // Segoe UI italique (« I »)
    Strike, // Segoe UI barré   (« S »)
    Icon,   // Segoe MDL2 Assets (icônes)
}

struct Def {
    id: usize,
    text: &'static str,
    font: F,
    chevron: bool,
    popup: bool,
    right: bool,
}

/// Icônes Segoe MDL2 Assets (valeurs officielles de la police système).
const GLYPH_LIST: &str = "\u{E8FD}"; // List
const GLYPH_LINK: &str = "\u{E71B}"; // Link
const GLYPH_TABLE: &str = "\u{E80A}"; // Grid
const GLYPH_FORMAT: &str = "\u{E8D2}"; // Font (« A » de mise en forme)
const GLYPH_WRITE: &str = "\u{E70F}"; // Edit (crayon, outils d'écriture)
const GLYPH_SHARE: &str = "\u{E72D}"; // Share
const GLYPH_ACCOUNT: &str = "\u{E77B}"; // Contact (personne dans un cercle)
const GLYPH_SETTINGS: &str = "\u{E713}"; // Setting (engrenage)
const CHEVRON: &str = "\u{E70D}"; // ChevronDown

const DEFS: &[Def] = &[
    Def { id: TB_H1, text: "H1", font: F::Text, chevron: true, popup: true, right: false },
    Def { id: TB_LIST, text: GLYPH_LIST, font: F::Icon, chevron: true, popup: true, right: false },
    Def { id: TB_BOLD, text: "B", font: F::Bold, chevron: false, popup: false, right: false },
    Def { id: TB_ITALIC, text: "I", font: F::Italic, chevron: false, popup: false, right: false },
    Def { id: TB_STRIKE, text: "S", font: F::Strike, chevron: false, popup: false, right: false },
    Def { id: TB_LINK, text: GLYPH_LINK, font: F::Icon, chevron: false, popup: false, right: false },
    Def { id: TB_TABLE, text: GLYPH_TABLE, font: F::Icon, chevron: true, popup: true, right: false },
    Def { id: TB_FORMAT, text: GLYPH_FORMAT, font: F::Icon, chevron: false, popup: true, right: false },
    Def { id: TB_WRITE, text: GLYPH_WRITE, font: F::Icon, chevron: true, popup: true, right: true },
    Def { id: TB_SHARE, text: GLYPH_SHARE, font: F::Icon, chevron: false, popup: false, right: true },
    Def { id: TB_ACCOUNT, text: GLYPH_ACCOUNT, font: F::Icon, chevron: false, popup: true, right: true },
    Def { id: TB_SETTINGS, text: GLYPH_SETTINGS, font: F::Icon, chevron: false, popup: true, right: true },
];

/// Marge horizontale autour de chaque élément (zone « Fluent » comme l'original).
/// Volontairement serré : le système ajoute déjà sa propre marge par élément.
const PADDING: i32 = 10;

thread_local! {
    static SPACER_W: Cell<i32> = const { Cell::new(40) };
    /// Largeur d'espaceur réellement appliquée au dernier rebuild (évite la
    /// boucle WM_SIZE → SetMenu → WM_SIZE … : on ne reconstruit que si elle change).
    static LAST_APPLIED: Cell<i32> = const { Cell::new(-1) };
    static IN_REFRESH: Cell<bool> = const { Cell::new(false) };
}

static mut FONTS: [HFONT; 6] = [HFONT(std::ptr::null_mut()); 6]; // text,bold,italic,strike,icon,chevron
static mut FONTS_READY: bool = false;
const F_CHEVRON_IDX: usize = 5;

fn font_for(f: F) -> HFONT {
    let idx = match f {
        F::Text => 0,
        F::Bold => 1,
        F::Italic => 2,
        F::Strike => 3,
        F::Icon => 4,
    };
    unsafe { FONTS[idx] }
}

unsafe fn make_fonts() {
    if FONTS_READY {
        return;
    }
    let mk = |height: i32, weight: i32, italic: bool, strike: bool, face: &str| -> HFONT {
        let mut lf: LOGFONTW = std::mem::zeroed();
        lf.lfHeight = height;
        lf.lfWeight = weight;
        lf.lfItalic = italic as u8;
        lf.lfStrikeOut = strike as u8;
        lf.lfCharSet = DEFAULT_CHARSET;
        lf.lfQuality = windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY;
        for (dst, c) in lf.lfFaceName.iter_mut().zip(face.encode_utf16()) {
            *dst = c;
        }
        CreateFontIndirectW(&lf)
    };
    FONTS[0] = mk(-12, 400, false, false, "Segoe UI");
    FONTS[1] = mk(-13, 700, false, false, "Segoe UI");
    FONTS[2] = mk(-13, 400, true, false, "Segoe UI");
    FONTS[3] = mk(-13, 400, false, true, "Segoe UI");
    FONTS[4] = mk(-14, 400, false, false, "Segoe MDL2 Assets");
    FONTS[5] = mk(-9, 400, false, false, "Segoe MDL2 Assets");
    FONTS_READY = true;
}

/// Largeur disponible pour l'espaceur = largeur cliente - groupes gauche/droit.
fn client_width() -> i32 {
    let a = app();
    if a.hwnd.is_invalid() {
        return 1200;
    }
    let mut rc = RECT::default();
    unsafe {
        GetClientRect(a.hwnd, &mut rc).ok();
    }
    rc.right.max(400)
}

/// Ajoute les éléments owner-draw après les menus classiques.
/// L'espaceur garde la valeur courante de SPACER_W (posée par refresh_menu).
pub fn append_items(menu: HMENU) {
    unsafe {
        make_fonts();
        for d in DEFS.iter().filter(|d| !d.right) {
            AppendMenuW(menu, MF_OWNERDRAW, d.id, PCWSTR::null()).ok();
        }
        AppendMenuW(menu, MF_OWNERDRAW, TB_SPACER, PCWSTR::null()).ok();
        for d in DEFS.iter().filter(|d| d.right) {
            AppendMenuW(menu, MF_OWNERDRAW, d.id, PCWSTR::null()).ok();
        }
    }
}

unsafe fn text_w(hdc: HDC, s: &str, font: HFONT) -> i32 {
    let old = SelectObject(hdc, font);
    let mut wide = to_wide(s);
    wide.pop(); // sans le NUL final pour une mesure exacte
    let mut sz = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, &wide, &mut sz);
    SelectObject(hdc, old);
    sz.cx
}

unsafe fn def_w(hdc: HDC, d: &Def) -> i32 {
    let mut total = text_w(hdc, d.text, font_for(d.font));
    if d.chevron {
        total += 3 + text_w(hdc, CHEVRON, FONTS[F_CHEVRON_IDX]);
    }
    total + PADDING
}

/// Réponse à WM_MEASUREITEM pour les éléments de la barre d'outils.
pub unsafe fn measure_item(mis: *mut MEASUREITEMSTRUCT) -> bool {
    let m = &mut *mis;
    if m.CtlType != ODT_MENU {
        return false;
    }
    let id = m.itemID as usize;
    if id == TB_SPACER {
        m.itemWidth = SPACER_W.get().max(24) as u32;
        m.itemHeight = 24;
        return true;
    }
    let Some(d) = DEFS.iter().find(|x| x.id == id) else {
        return false;
    };
    let a = app();
    let hdc = GetDC(a.hwnd);
    m.itemWidth = def_w(hdc, d) as u32;
    ReleaseDC(a.hwnd, hdc);
    m.itemHeight = 24;
    true
}

/// Réponse à WM_DRAWITEM : fond « barre de menus », surbrillance Fluent au survol,
/// glyphe centré + chevron, séparateur vertical au milieu de l'espaceur.
pub unsafe fn draw_item(dis: *const DRAWITEMSTRUCT) -> bool {
    let d = &*dis;
    if d.CtlType != ODT_MENU {
        return false;
    }
    let id = d.itemID as usize;
    let rc = d.rcItem;
    let _ = FillRect(d.hDC, &rc, GetSysColorBrush(COLOR_MENU));
    if id == TB_SPACER {
        // Séparateur vertical entre les deux groupes (comme l'original).
        let x = rc.left + (rc.right - rc.left) / 2;
        let sep = RECT {
            left: x,
            top: rc.top + 4,
            right: x + 1,
            bottom: rc.bottom - 4,
        };
        let _ = FillRect(d.hDC, &sep, GetSysColorBrush(COLOR_BTNFACE));
        return true;
    }
    let Some(def) = DEFS.iter().find(|x| x.id == id) else {
        return false;
    };
    if (d.itemState.0 & ODS_SELECTED.0) != 0 {
        let _ = FillRect(d.hDC, &rc, GetSysColorBrush(COLOR_MENUHILIGHT));
    }
    let old = SelectObject(d.hDC, font_for(def.font));
    let mut wide = to_wide(def.text);
    wide.pop();
    let mut sz = SIZE::default();
    let _ = GetTextExtentPoint32W(d.hDC, &wide, &mut sz);
    let cw = if def.chevron {
        text_w(d.hDC, CHEVRON, FONTS[F_CHEVRON_IDX])
    } else {
        0
    };
    let total = sz.cx + if def.chevron { 3 + cw } else { 0 };
    let mut x = rc.left + ((rc.right - rc.left - total) / 2).max(0);
    let mut r1 = RECT {
        left: x,
        top: rc.top,
        right: x + sz.cx,
        bottom: rc.bottom,
    };
    let _ = DrawTextW(d.hDC, &mut wide, &mut r1, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    if def.chevron {
        x += sz.cx + 3;
        let old2 = SelectObject(d.hDC, FONTS[F_CHEVRON_IDX]);
        let mut w2 = to_wide(CHEVRON);
        w2.pop();
        let mut r2 = RECT {
            left: x,
            top: rc.top,
            right: x + cw,
            bottom: rc.bottom,
        };
        let _ = DrawTextW(d.hDC, &mut w2, &mut r2, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        SelectObject(d.hDC, old2);
    }
    SelectObject(d.hDC, old);
    true
}

/// Exécute l'action d'un bouton (appelé depuis WM_COMMAND).
pub fn handle_command(id: usize) -> bool {
    if id == TB_SPACER {
        return true;
    }
    if DEFS.iter().all(|d| d.id != id) {
        return false;
    }
    unsafe {
        let r = item_rect(id);
        match id {
            TB_H1 => match popup_menu(&r, &[
                (6001, "Titre 1"), (6002, "Titre 2"), (6003, "Titre 3"),
                (6004, "Titre 4"), (6005, "Titre 5"), (6006, "Titre 6"),
                (6007, "Normal"),
            ]) {
                c @ 6001..=6006 => heading(c - 6000),
                6007 => heading(0),
                _ => {}
            },
            TB_LIST => match popup_menu(&r, &[
                (6011, "À puces"), (6012, "Numérotée"), (6013, "Case à cocher"),
            ]) {
                6011 => line_prefix("- "),
                6012 => line_prefix("1. "),
                6013 => line_prefix("- [ ] "),
                _ => {}
            },
            TB_TABLE => match popup_menu(&r, &[(6021, "2 × 2"), (6022, "3 × 3"), (6023, "4 × 4")]) {
                n @ 6021..=6023 => insert_table((n - 6020) as usize),
                _ => {}
            },
            TB_FORMAT => match popup_menu(&r, &[
                (6031, "MAJUSCULES"), (6032, "minuscules"), (6033, "Capitaliser Chaque Mot"),
            ]) {
                6031 => transform_selection(|c| c.to_uppercase().next().unwrap_or(c)),
                6032 => transform_selection(|c| c.to_lowercase().next().unwrap_or(c)),
                6033 => transform_selection(capitalize),
                _ => {}
            },
            TB_WRITE => {
                popup_menu(&r, &[(7001, "Vérification grammaticale"), (7002, "Suggestions")]);
            }
            TB_ACCOUNT => {
                popup_menu(&r, &[(7011, "Se connecter")]);
            }
            TB_SETTINGS => match popup_menu(&r, &[
                (6071, "Envoyer des commentaires"), (6072, "À propos du Bloc-notes"),
                (6073, "Obtenir des mises à jour"),
            ]) {
                6071 => post(cmd::FEEDBACK),
                6072 => post(cmd::ABOUT),
                _ => {}
            },
            TB_BOLD => wrap("**"),
            TB_ITALIC => wrap("*"),
            TB_STRIKE => wrap("~~"),
            TB_LINK => insert_link(),
            TB_SHARE => copy_path(),
            _ => {}
        }
    }
    true
}

/// Rectangle écran d'un élément de la barre de menus (position du bouton cliqué).
unsafe fn item_rect(id: usize) -> RECT {
    let a = app();
    let menu = GetMenu(a.hwnd);
    let n = GetMenuItemCount(menu);
    for i in 0..n.max(0) {
        if GetMenuItemID(menu, i) as usize == id {
            let mut r = RECT::default();
            if GetMenuItemRect(a.hwnd, menu, i as u32, &mut r).is_ok() {
                return r;
            }
        }
    }
    RECT {
        left: 80,
        top: 40,
        right: 160,
        bottom: 64,
    }
}

unsafe fn popup_menu(rect: &RECT, items: &[(usize, &str)]) -> usize {
    let menu = CreatePopupMenu().unwrap_or_default();
    for (id, label) in items {
        AppendMenuW(menu, MF_STRING, *id, pcw(&to_wide(label))).ok();
    }
    let cmd = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
        rect.left,
        rect.bottom + 2,
        0,
        app().hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
    cmd.0 as usize
}

fn post(id: usize) {
    unsafe {
        let _ = PostMessageW(app().hwnd, WM_COMMAND, WPARAM(id), LPARAM(0));
    }
}

/// Recalcule la largeur de l'espaceur et reconstruit la barre de menus (WM_SIZE).
/// La somme des largeurs RÉELLES des éléments (GetMenuItemRect, marges système
/// incluses) donne l'espaceur exact restant → le groupe droit touche le bord
/// droit comme l'original. Garde anti-réentrance + dernière largeur appliquée
/// pour ne pas retomber dans la boucle WM_SIZE → SetMenu → WM_SIZE.
pub fn refresh_menu() {
    let a = app();
    if a.hwnd.is_invalid() || !a.toolbar_visible {
        return;
    }
    if IN_REFRESH.with(|c| c.get()) {
        return;
    }
    IN_REFRESH.with(|c| c.set(true));
    let widths = unsafe { item_widths() };
    if let Some((sum, spacer_actual)) = widths {
        // Le système ajoute sa propre marge à CHAQUE élément, espaceur inclus :
        // on la déduit (mesurée : rectangle réel - largeur demandée) pour que
        // le total fasse exactement la largeur cliente (pas de retour à la ligne).
        let margin = (spacer_actual - SPACER_W.get()).max(0);
        let mut w = (client_width() - sum - margin).max(24);
        // Vérification adaptative : si un élément droit est passé à la ligne
        // suivante (capacité réelle du menu bar < largeur cliente), on réduit
        // l'espaceur jusqu'à tenir sur une seule ligne (comme l'original).
        for _ in 0..6 {
            if (w - LAST_APPLIED.with(|c| c.get())).abs() > 2 {
                SPACER_W.set(w);
                crate::app::rebuild_menus();
                LAST_APPLIED.with(|c| c.set(w));
            }
            if !unsafe { toolbar_wrapped() } {
                break;
            }
            w -= 40;
            if w <= 24 {
                w = 24;
                if (w - LAST_APPLIED.with(|c| c.get())).abs() > 2 {
                    SPACER_W.set(w);
                    crate::app::rebuild_menus();
                    LAST_APPLIED.with(|c| c.set(w));
                }
                break;
            }
        }
    }
    IN_REFRESH.with(|c| c.set(false));
}

/// true si le dernier bouton droit est dessiné sur une ligne du dessous
/// (débordement du menu bar).
unsafe fn toolbar_wrapped() -> bool {
    let a = app();
    let menu = GetMenu(a.hwnd);
    if menu.is_invalid() {
        return false;
    }
    let n = GetMenuItemCount(menu);
    let mut top_first: Option<i32> = None;
    for i in 0..n.max(0) {
        let id = GetMenuItemID(menu, i) as usize;
        if !(TB_H1..=TB_SETTINGS).contains(&id) {
            continue;
        }
        let mut r = RECT::default();
        if GetMenuItemRect(a.hwnd, menu, i as u32, &mut r).is_ok() {
            match top_first {
                None => top_first = Some(r.top),
                Some(t) if r.top > t + 4 => return true,
                _ => {}
            }
        }
    }
    false
}

/// (Somme des largeurs réelles hors espaceur, largeur réelle de l'espaceur).
/// None si la géométrie n'est pas encore disponible.
unsafe fn item_widths() -> Option<(i32, i32)> {
    let a = app();
    let menu = GetMenu(a.hwnd);
    if menu.is_invalid() {
        return None;
    }
    let n = GetMenuItemCount(menu);
    if n <= 0 {
        return None;
    }
    let mut sum = 0i32;
    let mut spacer_actual = 0i32;
    for i in 0..n {
        let mut r = RECT::default();
        if !GetMenuItemRect(a.hwnd, menu, i as u32, &mut r).is_ok() {
            return None;
        }
        let w = (r.right - r.left).max(0);
        if GetMenuItemID(menu, i) as usize == TB_SPACER {
            spacer_actual = w;
        } else {
            sum += w;
        }
    }
    Some((sum, spacer_actual))
}

// ---------------------------------------------------------------------------
// Actions Markdown (le mode édition de l'original insère du Markdown).
// ---------------------------------------------------------------------------

fn sel() -> (usize, usize) {
    let a = app();
    unsafe {
        let mut cr = CHARRANGE::default();
        SendMessageW(a.edit, EM_EXGETSEL, WPARAM(0), LPARAM(&mut cr as *mut _ as isize));
        (cr.cp_min.max(0) as usize, cr.cp_max.max(0) as usize)
    }
}

fn replace_with(text: &str) {
    let a = app();
    let wide = to_wide(text);
    unsafe {
        SendMessageW(a.edit, EM_REPLACESEL, WPARAM(1), LPARAM(wide.as_ptr() as isize));
    }
    crate::status::update();
}

fn set_sel(start: usize, end: usize) {
    let a = app();
    unsafe {
        let cr = CHARRANGE {
            cp_min: start as i32,
            cp_max: end as i32,
        };
        SendMessageW(
            a.edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&cr as *const _ as isize),
        );
    }
}

/// Entoure la sélection avec `mark` (ex. `**`), ou insère des marqueurs vides.
fn wrap(mark: &str) {
    let (s, e) = sel();
    if s == e {
        let text = format!("{mark}{mark}");
        replace_with(&text);
        set_sel(s + mark.chars().count(), s + mark.chars().count());
    } else {
        let full = crate::document::get_control_text(app().edit);
        let chars: Vec<char> = full.chars().collect();
        let (lo, hi) = (s.min(e), s.max(e).min(chars.len()));
        let inner: String = chars[lo..hi].iter().collect();
        replace_with(&format!("{mark}{inner}{mark}"));
        // Sélectionner le texte lui-même (sans les marqueurs), comme l'original :
        // un second clic de mise en forme s'applique au texte, pas aux marqueurs.
        let m = mark.chars().count();
        set_sel(lo + m, lo + m + inner.chars().count());
    }
}

fn insert_link() {
    let (s, e) = sel();
    if s == e {
        replace_with("[](https://)");
        set_sel(s + 1, s + 1);
    } else {
        let full = crate::document::get_control_text(app().edit);
        let chars: Vec<char> = full.chars().collect();
        let (lo, hi) = (s.min(e), s.max(e).min(chars.len()));
        let inner: String = chars[lo..hi].iter().collect();
        replace_with(&format!("[{inner}](https://)"));
    }
}

fn caret_line_start() -> usize {
    let a = app();
    let (s, _) = sel();
    unsafe {
        let line = crate::richedit_status::em_line_from_char(a.edit, s as i32);
        let start =
            SendMessageW(a.edit, EM_LINEINDEX, WPARAM(line.max(0) as usize), LPARAM(0)).0;
        start.max(0) as usize
    }
}

fn line_prefix(prefix: &str) {
    let start = caret_line_start();
    set_sel(start, start);
    replace_with(prefix);
}

fn heading(level: usize) {
    let start = caret_line_start();
    set_sel(start, start);
    if level == 0 {
        return;
    }
    replace_with(&format!("{} ", "#".repeat(level)));
}

fn insert_table(n: usize) {
    let mut head = String::new();
    let mut sep = String::new();
    let mut body = String::new();
    for c in 1..=n {
        head.push_str(&format!("| Colonne {c} "));
        sep.push_str("| --- ");
        body.push_str("|  ");
    }
    head.push('\n');
    sep.push('\n');
    body.push('\n');
    replace_with(&format!("{head}{sep}{body}"));
}

fn capitalize(c: char) -> char {
    c.to_ascii_uppercase()
}

fn transform_selection(f: fn(char) -> char) {
    let (s, e) = sel();
    if s == e {
        return;
    }
    let full = crate::document::get_control_text(app().edit);
    let chars: Vec<char> = full.chars().collect();
    let (lo, hi) = (s.min(e), s.max(e).min(chars.len()));
    let transformed: String = chars[lo..hi].iter().map(|c| f(*c)).collect();
    replace_with(&transformed);
    set_sel(lo, lo + transformed.chars().count());
}

fn copy_path() {
    let a = app();
    let path = a
        .docs
        .get(a.cur)
        .and_then(|d| d.path.as_ref())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Sans titre".to_string());
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if OpenClipboard(app().hwnd).is_ok() {
            let _ = EmptyClipboard();
            if let Ok(h) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
                let p = GlobalLock(h) as *mut u16;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                    let _ = GlobalUnlock(h);
                    // CF_UNICODETEXT == 13. HGLOBAL and HANDLE are both void*.
                    let _ = SetClipboardData(13, windows::Win32::Foundation::HANDLE(h.0));
                }
            }
            let _ = CloseClipboard();
        }
    }
}
