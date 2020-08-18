use crate::writer::DocWriter;
use async_std::io::prelude::Write;
use serde_pdf::ObjectId;
use std::hash::Hash;
use std::io;

#[async_trait::async_trait(?Send)]
pub trait FontCollection {
    type FontRef: Hash + Default + PartialEq + Eq + Clone + Copy;

    fn font(&self, font: Self::FontRef) -> &dyn Font;
    async fn write_objects<W: Write + Unpin>(
        &self,
        font: Self::FontRef,
        subset: SubsetRef,
        obj_id: ObjectId,
        doc: DocWriter<W>,
        compressed: bool,
    ) -> Result<DocWriter<W>, serde_pdf::Error>;
}

pub trait Font {
    fn base_name(&self) -> &str;
    fn kerning(&self, lhs: char, rhs: char) -> Option<i32>;
    fn encode_into(&self, text: &str, buf: &mut Vec<u8>) -> Result<(SubsetRef, usize), io::Error>;
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
