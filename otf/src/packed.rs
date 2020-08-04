use std::io;

pub trait Packed: Sized {
    type Dep;
    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error>;
    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error>;
}
