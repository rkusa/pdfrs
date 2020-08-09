use std::io;

use serde::Serialize;

pub struct Font(pub(super) FontVariant);

pub(super) enum FontVariant {
    #[cfg(feature = "afm")]
    Afm(super::afm::AfmFont),
    OpenType,
}

impl Font {
    pub fn base_name(&self) -> &str {
        match &self.0 {
            #[cfg(feature = "afm")]
            FontVariant::Afm(afm) => afm.base_name(),
            FontVariant::OpenType => unimplemented!(),
        }
    }

    pub fn object(&self) -> FontObject<'_> {
        match &self.0 {
            #[cfg(feature = "afm")]
            FontVariant::Afm(afm) => afm.object(),
            FontVariant::OpenType => unimplemented!(),
        }
    }

    pub fn kerning(&self, lhs: char, rhs: char) -> Option<i32> {
        match &self.0 {
            #[cfg(feature = "afm")]
            FontVariant::Afm(afm) => afm.kerning(lhs, rhs),
            FontVariant::OpenType => unimplemented!(),
        }
    }

    pub fn encode(&self, text: &str, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        match &self.0 {
            #[cfg(feature = "afm")]
            FontVariant::Afm(afm) => afm.encode(text, buf),
            FontVariant::OpenType => unimplemented!(),
        }
    }
}

#[derive(Serialize)]
pub enum FontType {
    Type1,
}

#[derive(Serialize)]
pub enum FontEncoding {
    WinAnsiEncoding,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "Font")]
pub struct FontObject<'a> {
    pub subtype: FontType,
    pub base_font: &'a str,
    pub encoding: FontEncoding,
}
