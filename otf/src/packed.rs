use std::io;

pub trait Packed: Sized {
    fn unpack<R: io::Read>(rd: &mut R) -> Result<Self, io::Error>;
    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error>;
}
