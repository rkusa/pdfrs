use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{self, Cursor};
use std::iter;

use super::glyf::GlyfTable;
use super::head::HeadTable;
use super::maxp::MaxpTable;
use super::{FontData, FontTable, Glyph};
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
    pub(crate) offsets: Vec<u32>,
    // not part of the font, but persisted to keep track of it
    pub(crate) format: Format,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum Format {
    Short,
    Long,
}

impl<'a> FontTable<'a, (&'a HeadTable, &'a MaxpTable), &'a GlyfTable, &'a GlyfTable> for LocaTable {
    fn name() -> &'static str {
        "loca"
    }
}

impl<'a> FontData<'a> for LocaTable {
    type UnpackDep = (&'a HeadTable, &'a MaxpTable);
    type PackDep = &'a GlyfTable;
    type SubsetDep = &'a GlyfTable;

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        (head, maxp): Self::UnpackDep,
    ) -> Result<Self, io::Error> {
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

    fn pack<W: io::Write>(&self, wr: &mut W, glyf: Self::PackDep) -> Result<(), io::Error> {
        let offsets = iter::once(0).chain(glyf.glyphs.iter().scan(0, |offset, data| {
            if let Some(data) = data {
                *offset += data.size_in_byte();
            }
            u32::try_from(*offset).ok()
        }));
        for offset in offsets {
            match self.format {
                Format::Short => wr.write_u16::<BigEndian>((offset / 2) as u16)?,
                Format::Long => wr.write_u32::<BigEndian>(offset)?,
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
    use super::*;
    use crate::tables::glyf::{GlyfTable, GlyphData, GlyphDescription};
    use crate::OffsetTable;

    #[test]
    fn test_loca_table_encode_decode() {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let head_table: HeadTable = table.unpack_required_table((), &mut cursor).unwrap();
        let maxp_table: MaxpTable = table.unpack_required_table((), &mut cursor).unwrap();
        let loca_table: LocaTable = table
            .unpack_required_table((&head_table, &maxp_table), &mut cursor)
            .unwrap();
        let glyf_table: GlyfTable = table
            .unpack_required_table(&loca_table, &mut cursor)
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
        loca_table.pack(&mut buffer, &glyf_table).unwrap();
        assert_eq!(
            LocaTable::unpack(&mut Cursor::new(&buffer[..]), (&head_table, &maxp_table)).unwrap(),
            loca_table
        );
    }

    #[test]
    fn test_loca_table_subset() {
        let g0 = GlyphData {
            number_of_contours: 0,
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
            description: GlyphDescription::Simple(vec![0; 5]),
        };
        let g1 = GlyphData {
            number_of_contours: 1,
            x_min: 1,
            y_min: 1,
            x_max: 1,
            y_max: 1,
            description: GlyphDescription::Simple(vec![0; 10]),
        };
        let g3 = GlyphData {
            number_of_contours: 3,
            x_min: 3,
            y_min: 3,
            x_max: 3,
            y_max: 3,
            description: GlyphDescription::Simple(vec![0; 20]),
        };
        let glyf = GlyfTable {
            glyphs: vec![Some(g0), Some(g1), None, None, Some(g3), None],
        };

        let loca = LocaTable {
            offsets: Vec::new(),
            format: Format::Long,
        };
        let subset = loca.subset(&[], &glyf);
        assert_eq!(
            subset.as_ref(),
            &LocaTable {
                offsets: vec![0, 15, 35, 35, 35, 65, 65],
                format: Format::Long,
            }
        )
    }
}
