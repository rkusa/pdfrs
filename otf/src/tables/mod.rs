pub mod cmap;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod maxp;
pub mod name;
pub mod os2;
pub mod post;

use std::borrow::Cow;
use std::io;

pub trait FontTable<'a>: Sized {
    type Dep;
    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error>;
    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error>;
    fn subset(&'a self, _glyph_ids: &[u16]) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Borrowed(self)
    }
}
