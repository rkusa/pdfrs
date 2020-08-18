use std::io;

use crate::fonts::font::{Font, FontCollection, SingleFont, SubsetRef};
use crate::writer::DocWriter;
use async_std::io::prelude::Write;
use serde::Serialize;
use serde_pdf::{Object, ObjectId, PdfStr};

impl<'a> Font for &'a pdfrs_afm::AfmFont {
    fn base_name(&self) -> &str {
        self.font_name
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
#[async_trait::async_trait(?Send)]
impl<'a> FontCollection for &'a pdfrs_afm::AfmFont {
    type FontRef = SingleFont;

    fn font(&self, _font: Self::FontRef) -> &dyn Font {
        self
    }

    async fn write_objects<W: Write + Unpin>(
        &self,
        _font: Self::FontRef,
        _subset: SubsetRef,
        obj_id: ObjectId,
        mut doc: DocWriter<W>,
        _: bool,
    ) -> Result<DocWriter<W>, serde_pdf::Error> {
        let font_obj = Object::new(
            obj_id.id(),
            obj_id.rev(),
            FontObject {
                subtype: FontType::Type1,
                base_font: self.base_name(),
                encoding: FontEncoding::WinAnsiEncoding,
            },
        );
        doc.write_object(font_obj).await?;
        Ok(doc)
    }
}

#[derive(Serialize)]
enum FontType {
    Type1,
}

#[derive(Serialize)]
enum FontEncoding {
    WinAnsiEncoding,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "Font")]
struct FontObject<'a> {
    subtype: FontType,
    base_font: &'a str,
    encoding: FontEncoding,
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
