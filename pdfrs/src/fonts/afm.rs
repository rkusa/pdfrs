// TODO: remove once AFM fonts are implemented
#![allow(unused)]

use super::font::{FontEncoding, FontObject, FontType};
use super::Font;
use std::collections::HashMap;

#[derive(Default)]
pub struct AfmFont {
    pub(crate) cap_height: i32,
    pub(crate) x_height: i32,
    pub(crate) ascender: i32,
    pub(crate) descender: i32,
    pub(crate) italic_angle: f32,
    pub(crate) underline_position: i32,
    pub(crate) underline_thickness: i32,
    pub(crate) font_bbox: (i32, i32, i32, i32),
    pub(crate) font_name: &'static str,
    pub(crate) full_name: &'static str,
    pub(crate) family_name: &'static str,
    pub(crate) character_set: &'static str,
    pub(crate) glyph_widths: HashMap<u8, u32>,
    pub(crate) kerning: HashMap<(u32, u32), i32>,
}

impl Font for AfmFont {
    fn base_name(&self) -> &str {
        self.font_name
    }

    fn object(&self) -> FontObject<'_> {
        FontObject {
            subtype: FontType::Type1,
            base_font: self.base_name(),
            encoding: FontEncoding::WinAnsiEncoding,
        }
    }
}
