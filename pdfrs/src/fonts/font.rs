use std::io;

use serde::Serialize;

pub trait Font {
    fn base_name(&self) -> &str;
    fn object(&self) -> FontObject<'_>;
    fn kerning(&self, lhs: char, rhs: char) -> Option<i32>;
    fn encode(&self, text: &str, buf: &mut Vec<u8>) -> Result<(), io::Error>;
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
