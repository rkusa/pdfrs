use std::io;

use crate::packed::Packed;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, PartialEq)]
pub struct Format12 {
    language: u32,
    num_groups: u32,
    sequential_map_groups: Vec<SequentialMapGroup>,
}

impl Packed for Format12 {
    fn unpack<R: io::Read>(mut rd: &mut R) -> Result<Self, io::Error> {
        let language = rd.read_u32::<BigEndian>()?;
        let num_groups = rd.read_u32::<BigEndian>()?;

        let mut groups = Vec::with_capacity(num_groups as usize);
        for _ in 0..num_groups {
            groups.push(SequentialMapGroup::unpack(&mut rd)?);
        }

        Ok(Format12 {
            language,
            num_groups,
            sequential_map_groups: groups,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(self.language)?;
        wr.write_u32::<BigEndian>(self.sequential_map_groups.len() as u32)?;
        for group in &self.sequential_map_groups {
            group.pack(&mut wr)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct SequentialMapGroup {
    start_char_code: u32,
    end_char_code: u32,
    start_glyph_id: u32,
}

impl Packed for SequentialMapGroup {
    fn unpack<R: io::Read>(rd: &mut R) -> Result<Self, io::Error> {
        Ok(SequentialMapGroup {
            start_char_code: rd.read_u32::<BigEndian>()?,
            end_char_code: rd.read_u32::<BigEndian>()?,
            start_glyph_id: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(self.start_char_code)?;
        wr.write_u32::<BigEndian>(self.end_char_code)?;
        wr.write_u32::<BigEndian>(self.start_glyph_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::tables::cmap::{CmapTable, Subtable};
    use crate::OffsetTable;

    #[test]
    fn test_cmap_subtable_format12_encode_decode() {
        let data = include_bytes!("../../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor).unwrap();
        let cmap_record = table.get_table_record("cmap").unwrap();
        let cmap_table: CmapTable = table.unpack_required_table("cmap", &mut cursor).unwrap();

        let record = cmap_table
            .encoding_records
            .iter()
            .find(|r| r.platform_id == 0 && r.encoding_id == 4)
            .unwrap();

        cursor.set_position((cmap_record.offset + record.offset) as u64);
        let subtable = Subtable::unpack(&mut cursor).unwrap();
        let format12 = match subtable {
            Subtable::Format12(subtable) => subtable,
            _ => panic!("Expected format 12 subtable"),
        };

        assert_eq!(
            format12.sequential_map_groups.len(),
            format12.num_groups as usize
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        format12.pack(&mut buffer).unwrap();
        assert_eq!(
            Format12::unpack(&mut Cursor::new(buffer)).unwrap(),
            format12
        );
    }
}
