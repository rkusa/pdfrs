use crate::packed::Packed;
use std::io;

#[derive(Debug, PartialEq)]
pub struct Format12 {}

impl Packed for Format12 {
    fn unpack<R: io::Read>(_rd: &mut R) -> Result<Self, io::Error> {
        unimplemented!()
    }

    fn pack<W: io::Write>(&self, _wr: &mut W) -> Result<(), io::Error> {
        unimplemented!()
    }
}
