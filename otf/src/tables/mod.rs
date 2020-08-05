pub mod cmap;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod maxp;
pub mod name;
pub mod os2;
pub mod post;

use std::io;

pub trait FontTable<'a>: Sized {
    type Dep;
    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error>;
    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error>;
}
