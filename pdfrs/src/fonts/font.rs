use serde::Serialize;

pub trait Font {
    fn base_name(&self) -> &str;
    fn object(&self) -> FontObject<'_>;
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
