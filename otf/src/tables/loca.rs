use std::borrow::Cow;
use std::convert::TryFrom;
use std::{io, iter};

use super::glyf::GlyfTable;
use super::head::HeadTable;
use super::maxp::MaxpTable;
use super::{FontTable, Glyph};
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

#[derive(Debug, PartialEq, Clone, Copy)]
pub(super) enum Format {
    Short,
    Long,
}

impl<'a> FontTable<'a> for LocaTable {
    type UnpackDep = (&'a HeadTable, &'a MaxpTable);
    type SubsetDep = &'a GlyfTable;

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

    fn subset(&'a self, _glyphs: &[Glyph], glyf: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Owned(LocaTable {
            offsets: iter::once(0)
                .chain(glyf.glyphs.iter().scan(0, |offset, data| {
                    if let Some(data) = data {
                        *offset += data.size_in_byte();
                    }
                    u32::try_from(*offset).ok()
                }))
                .collect(),
            format: self.format,
        })
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::tables::glyf::GlyphData;
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

    #[test]
    fn test_loca_table_subset() {
        let g1 = GlyphData {
            number_of_contours: 1,
            x_min: 1,
            y_min: 1,
            x_max: 1,
            y_max: 1,
            description: vec![0; 10],
        };
        let g3 = GlyphData {
            number_of_contours: 3,
            x_min: 3,
            y_min: 3,
            x_max: 3,
            y_max: 3,
            description: vec![0; 20],
        };
        let glyf = GlyfTable {
            glyphs: vec![Some(g1), None, None, Some(g3), None],
        };

        let loca = LocaTable {
            offsets: Vec::new(),
            format: Format::Long,
        };
        let subset = loca.subset(&[], &glyf);
        assert_eq!(
            subset.as_ref(),
            &LocaTable {
                offsets: vec![0, 20, 20, 20, 50, 50],
                format: Format::Long,
            }
        )
    }
}
