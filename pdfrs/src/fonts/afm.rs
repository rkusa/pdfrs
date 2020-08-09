use std::io;

use crate::fonts::font::{Font, FontEncoding, FontObject, FontType, FontVariant};
use once_cell::sync::Lazy;
use serde_pdf::PdfStr;

pub struct AfmFont(&'static pdfrs_afm::AfmFont);

#[cfg(feature = "courier_bold")]
pub static COURIER_BOLD: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::COURIER_BOLD))));

#[cfg(feature = "courier_bold_oblique")]
pub static COURIER_BOLD_OBLIQUE: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::COURIER_BOLD_OBLIQUE))));

#[cfg(feature = "courier_oblique")]
pub static COURIER_OBLIQUE: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::COURIER_OBLIQUE))));

#[cfg(feature = "courier")]
pub static COURIER: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::COURIER))));

#[cfg(feature = "helvetica_bold")]
pub static HELVETICA_BOLD: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::HELVETICA_BOLD))));

#[cfg(feature = "helvetica_bold_oblique")]
pub static HELVETICA_BOLD_OBLIQUE: Lazy<Font> = Lazy::new(|| {
    Font(FontVariant::Afm(AfmFont(
        &*pdfrs_afm::HELVETICA_BOLD_OBLIQUE,
    )))
});

#[cfg(feature = "helvetica_oblique")]
pub static HELVETICA_OBLIQUE: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::HELVETICA_OBLIQUE))));

#[cfg(feature = "helvetica")]
pub static HELVETICA: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::HELVETICA))));

#[cfg(feature = "symbol")]
pub static SYMBOL: Lazy<Font> = Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::SYMBOL))));

#[cfg(feature = "times_bold")]
pub static TIMES_BOLD: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::TIMES_BOLD))));

#[cfg(feature = "times_bold_italic")]
pub static TIMES_BOLD_ITALIC: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::TIMES_BOLD_ITALIC))));

#[cfg(feature = "times_italic")]
pub static TIMES_ITALIC: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::TIMES_ITALIC))));

#[cfg(feature = "times_roman")]
pub static TIMES_ROMAN: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::TIMES_ROMAN))));

#[cfg(feature = "zapf_dingbats")]
pub static ZAPF_DINGBATS: Lazy<Font> =
    Lazy::new(|| Font(FontVariant::Afm(AfmFont(&*pdfrs_afm::ZAPF_DINGBATS))));

impl AfmFont {
    pub fn base_name(&self) -> &str {
        self.0.font_name
    }

    pub fn object(&self) -> FontObject<'_> {
        FontObject {
            subtype: FontType::Type1,
            base_font: self.base_name(),
            encoding: FontEncoding::WinAnsiEncoding,
        }
    }

    pub fn kerning(&self, lhs: char, rhs: char) -> Option<i32> {
        self.0.kerning.get(&(lhs as u32, rhs as u32)).cloned()
    }

    pub fn encode(&self, text: &str, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        buf.clear();
        buf.extend_from_slice(PdfStr::Literal(text).to_string().as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::fonts::afm::HELVETICA;
    use crate::fonts::Font;

    #[test]
    fn test_encode_basic() {
        let mut buf = Vec::new();
        HELVETICA.encode("Hello", &mut buf).unwrap();
        assert_eq!(buf.as_slice(), b"(Hello)");
        assert_eq!(&String::from_utf8_lossy(&buf), "(Hello)");
    }

    #[test]
    fn test_encode_reserved_characters() {
        let mut buf = Vec::new();
        HELVETICA.encode("Hello \\(World)", &mut buf).unwrap();
        assert_eq!(&String::from_utf8_lossy(&buf), "(Hello \\\\\\(World\\))");
    }
}
