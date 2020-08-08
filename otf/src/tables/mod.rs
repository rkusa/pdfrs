pub mod cmap;
pub mod glyf;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod loca;
pub mod maxp;
pub mod name;
pub mod os2;
pub mod post;

use std::borrow::Cow;
use std::io;

pub trait FontTable<'a>: Sized {
    type UnpackDep;
    type SubsetDep;

    fn unpack<R: io::Read>(rd: &mut R, _dep: Self::UnpackDep) -> Result<Self, io::Error>;
    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error>;

    fn subset(&'a self, _glyphs: &[Glyph], _dep: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Borrowed(self)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Glyph {
    pub index: u16,
    pub code_points: Vec<u32>,
}

impl Glyph {
    pub fn new(index: u16) -> Self {
        Glyph {
            index,
            code_points: Vec::new(),
        }
    }
}

impl From<u16> for Glyph {
    fn from(index: u16) -> Self {
        Glyph::new(index)
    }
}
