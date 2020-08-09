//! This module contains built-in PDF (AFM) fonts. When using those fonts, it is not necessary
//! to embed a font into the document, since each PDF reader provides them. However, compared to
//! embedding fonts, those AFM fonts only have a limited set of available characters.
//!
//! Each AFM font needs to be enabled via a Cargo feature. Only `HELVETICA` is enabled as part of
//! the default feature set. The following features for enabling AFM fonts are available:
//! `courier_bold`, `courier_bold_oblique`, `courier_oblique`, `courier`, `helvetica_bold`,
//! `helvetica_bold_oblique`, `helvetica_oblique`, `helvetica`, `symbol`, `times_bold`,
//! `times_bold_italic`, `times_italic`, `times_roman`, `zapf_dingbats`.

use std::collections::HashMap;

#[derive(Default)]
pub struct AfmFont {
    pub cap_height: i32,
    pub x_height: i32,
    pub ascender: i32,
    pub descender: i32,
    pub italic_angle: f32,
    pub underline_position: i32,
    pub underline_thickness: i32,
    pub font_bbox: (i32, i32, i32, i32),
    pub font_name: &'static str,
    pub full_name: &'static str,
    pub family_name: &'static str,
    pub character_set: &'static str,
    pub glyph_widths: HashMap<u8, u32>,
    pub kerning: HashMap<(u32, u32), i32>,
}

#[cfg(feature = "courier_bold")]
include!(concat!(env!("OUT_DIR"), "/courier_bold.rs"));
#[cfg(feature = "courier_bold_oblique")]
include!(concat!(env!("OUT_DIR"), "/courier_bold_oblique.rs"));
#[cfg(feature = "courier_oblique")]
include!(concat!(env!("OUT_DIR"), "/courier_oblique.rs"));
#[cfg(feature = "courier")]
include!(concat!(env!("OUT_DIR"), "/courier.rs"));
#[cfg(feature = "helvetica_bold")]
include!(concat!(env!("OUT_DIR"), "/helvetica_bold.rs"));
#[cfg(feature = "helvetica_bold_oblique")]
include!(concat!(env!("OUT_DIR"), "/helvetica_bold_oblique.rs"));
#[cfg(feature = "helvetica_oblique")]
include!(concat!(env!("OUT_DIR"), "/helvetica_oblique.rs"));
#[cfg(feature = "helvetica")]
include!(concat!(env!("OUT_DIR"), "/helvetica.rs"));
#[cfg(feature = "symbol")]
include!(concat!(env!("OUT_DIR"), "/symbol.rs"));
#[cfg(feature = "times_bold")]
include!(concat!(env!("OUT_DIR"), "/times_bold.rs"));
#[cfg(feature = "times_bold_italic")]
include!(concat!(env!("OUT_DIR"), "/times_bold_italic.rs"));
#[cfg(feature = "times_italic")]
include!(concat!(env!("OUT_DIR"), "/times_italic.rs"));
#[cfg(feature = "times_roman")]
include!(concat!(env!("OUT_DIR"), "/times_roman.rs"));
#[cfg(feature = "zapf_dingbats")]
include!(concat!(env!("OUT_DIR"), "/zapf_dingbats.rs"));
