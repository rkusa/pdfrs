use std::io;

use super::head::HeadTable;
use super::maxp::MaxpTable;
use super::FontTable;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table stores the offsets to the locations of the glyphs in the font, relative to the
/// beginning of the glyph data table.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/loca
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6loca.html
#[derive(Debug, PartialEq, Clone)]
pub struct LocaTable {
    /// Offsets indexed by glyph id. The size of a glyph data block is inferred from the difference
    /// between two consecutive offsets.
    pub(super) offsets: Vec<u32>,
    // not part of the font, but persisted to keep track of it
    pub(super) format: Format,
}

#[derive(Debug, PartialEq, Clone)]
pub(super) enum Format {
    Short,
    Long,
}

impl<'a> FontTable<'a> for LocaTable {
    type UnpackDep = (&'a HeadTable, &'a MaxpTable);
    type SubsetDep = ();

    fn unpack<R: io::Read>(rd: &mut R, (head, maxp): Self::UnpackDep) -> Result<Self, io::Error> {
        let n = maxp.num_glyphs() as usize + 1;
        let mut offsets = Vec::with_capacity(n);
        for _ in 0..n {
            offsets.push(if head.index_to_loc_format == 0 {
                (rd.read_u16::<BigEndian>()? as u32) * 2
            } else {
                rd.read_u32::<BigEndian>()?
            });
        }

        Ok(LocaTable {
            offsets,
            format: if head.index_to_loc_format == 0 {
                Format::Short
            } else {
                Format::Long
            },
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        for offset in &self.offsets {
            match self.format {
                Format::Short => wr.write_u16::<BigEndian>((offset / 2) as u16)?,
                Format::Long => wr.write_u32::<BigEndian>(*offset)?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_loca_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let head_table: HeadTable = table
            .unpack_required_table("head", (), &mut cursor)
            .unwrap();
        let maxp_table: MaxpTable = table
            .unpack_required_table("maxp", (), &mut cursor)
            .unwrap();
        let loca_table: LocaTable = table
            .unpack_required_table("loca", (&head_table, &maxp_table), &mut cursor)
            .unwrap();

        assert_eq!(
            loca_table.offsets.len(),
            (maxp_table.num_glyphs() as usize) + 1
        );
        assert_eq!(
            loca_table.format,
            if head_table.index_to_loc_format == 0 {
                Format::Short
            } else {
                Format::Long
            }
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        loca_table.pack(&mut buffer).unwrap();
        assert_eq!(
            LocaTable::unpack(&mut Cursor::new(buffer), (&head_table, &maxp_table)).unwrap(),
            loca_table
        );
    }
}
