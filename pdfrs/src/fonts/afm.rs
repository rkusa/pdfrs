use std::io;

use crate::fonts::font::{
    Font, FontCollection, FontEncoding, FontObject, FontType, SingleFont, SubsetRef,
};
use serde_pdf::PdfStr;

impl<'a> Font for &'a pdfrs_afm::AfmFont {
    fn base_name(&self) -> &str {
        self.font_name
    }

    fn object(&self) -> FontObject {
        FontObject {
            subtype: FontType::Type1,
            base_font: self.base_name().to_string(),
            encoding: FontEncoding::WinAnsiEncoding,
        }
    }

    fn kerning(&self, lhs: char, rhs: char) -> Option<i32> {
        self.kerning.get(&(lhs as u32, rhs as u32)).cloned()
    }

    fn encode_into(&self, text: &str, buf: &mut Vec<u8>) -> Result<(SubsetRef, usize), io::Error> {
        buf.clear();
        buf.extend_from_slice(PdfStr::Literal(text).to_string().as_bytes());
        Ok((SubsetRef(0), text.len()))
    }
}

#[cfg(any(feature = "afm", test))]
impl<'a> FontCollection for &'a pdfrs_afm::AfmFont {
    type FontRef = SingleFont;

    fn font(&self, _font: Self::FontRef) -> &dyn Font {
        self
    }
}

#[cfg(test)]
mod test {
    use crate::fonts::{Font, HELVETICA};
    use std::ops::Deref;

    #[test]
    fn test_encode_basic() {
        let mut buf = Vec::new();
        HELVETICA.deref().encode_into("Hello", &mut buf).unwrap();
        assert_eq!(buf.as_slice(), b"(Hello)");
        assert_eq!(&String::from_utf8_lossy(&buf), "(Hello)");
    }

    #[test]
    fn test_encode_reserved_characters() {
        let mut buf = Vec::new();
        HELVETICA
            .deref()
            .encode_into("Hello \\(World)", &mut buf)
            .unwrap();
        assert_eq!(&String::from_utf8_lossy(&buf), "(Hello \\\\\\(World\\))");
    }
}
