use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{self, Cursor};

use super::head::HeadTable;
use super::hmtx::HmtxTable;
use super::{FontData, FontTable, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// The Glyph Positioning table (GPOS) provides precise control over glyph placement for
/// sophisticated text layout and rendering in each script and language system that a font supports.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/gpos
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(Default))]
pub struct GposTable {}

impl<'a> FontTable<'a, (), (), ()> for GposTable {
    fn name() -> &'static str {
        "GPOS"
    }
}

impl<'a> FontData<'a> for GposTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        unimplemented!()
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        Ok(())
    }
}
