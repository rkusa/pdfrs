use std::hash::Hash;
use std::io;

use serde::Serialize;

pub trait FontCollection {
    type FontRef: Hash + Default + PartialEq + Eq + Clone + Copy;

    fn font(&self, font: Self::FontRef) -> &dyn Font;
}

pub trait Font {
    fn base_name(&self) -> &str;
    fn object(&self) -> FontObject;
    fn kerning(&self, lhs: char, rhs: char) -> Option<i32>;
    fn encode_into(&self, text: &str, buf: &mut Vec<u8>) -> Result<(SubsetRef, usize), io::Error>;
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
pub struct FontObject {
    pub subtype: FontType,
    pub base_font: String,
    pub encoding: FontEncoding,
}

#[derive(Hash, Default, PartialEq, Eq, Clone, Copy)]
pub struct SingleFont(pub(super) usize);

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct SubsetRef(pub(super) usize);

impl SubsetRef {
    pub fn font_id(&self) -> usize {
        self.0
    }
}
