use std::io::{self, Read};
use std::mem;

use super::loca::LocaTable;
use super::FontTable;
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// The 'glyf' table is comprised of a list of glyph data blocks, each of which provides the
/// description for a single glyph. Glyphs are referenced by identifiers (glyph IDs), which are
/// sequential integers beginning at zero.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/glyf
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6glyf.html
#[derive(Debug, PartialEq, Clone)]
pub struct GlyfTable {
    glyphs: Vec<Option<GlyphData>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct GlyphData {
    /// If the number of contours is greater than or equal to zero, this is a simple glyph. If
    /// negative, this is a composite glyph — the value -1 should be used for composite glyphs.
    number_of_contours: i16,
    /// Minimum x for coordinate data.
    x_min: i16,
    /// Minimum y for coordinate data.
    y_min: i16,
    /// Maximum x for coordinate data.
    x_max: i16,
    /// Maximum y for coordinate data.
    y_max: i16,
    /// The raw glyph description.
    // TODO: parse into actual simple/composit enum?
    description: Vec<u8>,
}

impl<'a> FontTable<'a> for GlyfTable {
    type UnpackDep = &'a LocaTable;
    type SubsetDep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, loca: Self::UnpackDep) -> Result<Self, io::Error> {
        dbg!(&loca.offsets);
        let mut glyphs = Vec::with_capacity(loca.offsets.len().saturating_sub(1));

        let mut pos = 0;
        for (start, end) in loca.offsets.iter().zip(loca.offsets.iter().skip(1)) {
            let start = *start as usize;
            let end = *end as usize;

            if start == end {
                // glyph has no outline
                glyphs.push(None);
                continue;
            }

            if start > pos {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Encountered unaligned LOCA table offsets",
                ));
            }

            dbg!((start, end, end - start));

            let mut limit_read = LimitRead::new(&mut rd, end - start);
            let number_of_contours = limit_read.read_i16::<BigEndian>()?;
            let x_min = limit_read.read_i16::<BigEndian>()?;
            let y_min = limit_read.read_i16::<BigEndian>()?;
            let x_max = limit_read.read_i16::<BigEndian>()?;
            let y_max = limit_read.read_i16::<BigEndian>()?;

            let mut description = Vec::with_capacity(end - start - mem::size_of::<i16>() * 5);
            limit_read.read_to_end(&mut description)?;

            glyphs.push(Some(GlyphData {
                number_of_contours,
                x_min,
                y_min,
                x_max,
                y_max,
                description,
            }));

            pos = end;
        }

        Ok(GlyfTable { glyphs })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        for data in &self.glyphs {
            if let Some(data) = data {
                wr.write_i16::<BigEndian>(data.number_of_contours)?;
                wr.write_i16::<BigEndian>(data.x_min)?;
                wr.write_i16::<BigEndian>(data.y_min)?;
                wr.write_i16::<BigEndian>(data.x_max)?;
                wr.write_i16::<BigEndian>(data.y_max)?;
                wr.write_all(&data.description)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::tables::head::HeadTable;
    use crate::tables::maxp::MaxpTable;
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
        let glyf_table: GlyfTable = table
            .unpack_required_table("glyf", &loca_table, &mut cursor)
            .unwrap();

        assert_eq!(
            glyf_table.glyphs.len(),
            (loca_table.offsets.len() as usize) - 1
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        glyf_table.pack(&mut buffer).unwrap();
        assert_eq!(
            GlyfTable::unpack(&mut Cursor::new(buffer), &loca_table).unwrap(),
            glyf_table
        );
    }
}
