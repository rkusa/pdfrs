// TODO: remove once AFM fonts are implemented
#![allow(unused)]

use std::collections::HashMap;
use std::io::{self, Write};

use super::font::{FontEncoding, FontObject, FontType};
use super::Font;

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

    fn kerning(&self, lhs: char, rhs: char) -> Option<i32> {
        self.kerning.get(&(lhs as u32, rhs as u32)).cloned()
    }

    fn encode(&self, text: &str, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        buf.clear();
        buf.push(b'(');
        for c in text.chars() {
            match c {
                '\\' => buf.extend_from_slice("\\\\".as_bytes()),
                '(' => buf.extend_from_slice("\\(".as_bytes()),
                ')' => buf.extend_from_slice("\\)".as_bytes()),
                c => buf.push(c as u8),
            }
        }
        buf.push(b')');
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::fonts::{Font, HELVETICA};

    #[test]
    fn encode_basic() {
        let mut buf = Vec::new();
        HELVETICA.encode("Hello", &mut buf).unwrap();
        assert_eq!(buf.as_slice(), b"(Hello)");
        assert_eq!(&String::from_utf8_lossy(&buf), "(Hello)");
    }

    #[test]
    fn encode_reserved_characters() {
        let mut buf = Vec::new();
        HELVETICA.encode("Hello \\(World)", &mut buf).unwrap();
        assert_eq!(&String::from_utf8_lossy(&buf), "(Hello \\\\\\(World\\))");
    }
}
