//! Persistent settings, stored in the very same registry values as the original
//! (iWindowPosX / iWindowPosY / iWindowPosDX / iWindowPosDY, fSaveWindowPositions,
//!  IsWordWrap, StatusBarVisibility, fWindowsOnlyEOL, DefaultFonts, ...).

use crate::reg;
use crate::{REG_KEY, REG_KEY_FONTS};

#[derive(Clone)]
pub struct Settings {
    pub win_x: i32,
    pub win_y: i32,
    pub win_w: i32,
    pub win_h: i32,
    pub save_positions: bool,
    pub word_wrap: bool,
    pub status_bar: bool,
    /// true => new documents use CRLF (fWindowsOnlyEOL), false => LF
    pub windows_eol: bool,
    pub font_face: String,
    pub font_height: i32,
    pub font_weight: u32,
    pub font_italic: bool,
    pub font_underline: bool,
    pub zoom: u32,
    /// Barre d'outils de mise en forme visible (mode édition), comme l'original.
    pub toolbar_visible: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            win_x: 0,
            win_y: 0,
            win_w: 0,
            win_h: 0,
            save_positions: true,
            word_wrap: true,
            status_bar: true,
            windows_eol: true,
            font_face: "Consolas".to_string(),
            font_height: -15,
            font_weight: 400,
            font_italic: false,
            font_underline: false,
            zoom: 100,
            toolbar_visible: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        s.win_x = reg::get_dword(REG_KEY, "iWindowPosX", 0) as i32;
        s.win_y = reg::get_dword(REG_KEY, "iWindowPosY", 0) as i32;
        s.win_w = reg::get_dword(REG_KEY, "iWindowPosDX", 1121) as i32;
        s.win_h = reg::get_dword(REG_KEY, "iWindowPosDY", 611) as i32;
        s.save_positions = reg::get_dword(REG_KEY, "fSaveWindowPositions", 1) != 0;
        s.word_wrap = reg::get_dword(REG_KEY, "IsWordWrap", 1) != 0;
        s.status_bar = reg::get_dword(REG_KEY, "StatusBarVisibility", 1) != 0;
        s.windows_eol = reg::get_dword(REG_KEY, "fWindowsOnlyEOL", 1) != 0;
        s.toolbar_visible = reg::get_dword(REG_KEY, "IsFormattingToolbarVisible", 1) != 0;
        s.font_face = reg::get_sz(REG_KEY_FONTS, "lfFaceName", "Consolas");
        s.font_height = reg::get_dword(REG_KEY_FONTS, "lfHeight", 15) as i32;
        if s.font_height > 0 {
            s.font_height = -s.font_height;
        }
        s.font_weight = reg::get_dword(REG_KEY_FONTS, "lfWeight", 400);
        s.font_italic = reg::get_dword(REG_KEY_FONTS, "lfItalic", 0) != 0;
        s.font_underline = reg::get_dword(REG_KEY_FONTS, "lfUnderline", 0) != 0;
        s.zoom = {
            let z = reg::get_dword(REG_KEY, "ZoomLevel", 100);
            if (10..=400).contains(&z) {
                z
            } else {
                100
            }
        };
        s
    }

    pub fn save(&self) {
        reg::set_dword(REG_KEY, "iWindowPosX", self.win_x as u32);
        reg::set_dword(REG_KEY, "iWindowPosY", self.win_y as u32);
        reg::set_dword(REG_KEY, "iWindowPosDX", self.win_w as u32);
        reg::set_dword(REG_KEY, "iWindowPosDY", self.win_h as u32);
        reg::set_dword(REG_KEY, "fSaveWindowPositions", self.save_positions as u32);
        reg::set_dword(REG_KEY, "IsWordWrap", self.word_wrap as u32);
        reg::set_dword(REG_KEY, "StatusBarVisibility", self.status_bar as u32);
        reg::set_dword(REG_KEY, "fWindowsOnlyEOL", self.windows_eol as u32);
        reg::set_dword(REG_KEY, "IsFormattingToolbarVisible", self.toolbar_visible as u32);
        reg::set_sz(REG_KEY_FONTS, "lfFaceName", &self.font_face);
        reg::set_dword(REG_KEY_FONTS, "lfHeight", self.font_height.unsigned_abs());
        reg::set_dword(REG_KEY_FONTS, "lfWeight", self.font_weight);
        reg::set_dword(REG_KEY_FONTS, "lfItalic", self.font_italic as u32);
        reg::set_dword(REG_KEY_FONTS, "lfUnderline", self.font_underline as u32);
        reg::set_dword(REG_KEY, "ZoomLevel", self.zoom);
    }
}
