use std::convert::TryFrom;
use std::io;

use crate::packed::Packed;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, PartialEq)]
pub struct Format12 {
    language: u32,
    num_groups: u32,
    sequential_map_groups: Vec<SequentialMapGroup>,
}

impl Format12 {
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        let search_result = self
            .sequential_map_groups
            .binary_search_by_key(&codepoint, |group| group.end_char_code);
        let ix = match search_result {
            // Found a direct match
            Ok(ix) => ix,
            // No direct match, `ix` represents the position where the codepoint could be inserted
            // while maintaining sorted order -> the index represents the first end code that is
            // greater than the character code
            Err(ix) => ix,
        };
        let group = self.sequential_map_groups.get(ix)?;
        dbg!(codepoint);
        dbg!(ix);
        dbg!(&group);
        if codepoint < group.start_char_code || codepoint > group.end_char_code {
            return None;
        }

        u16::try_from(
            group
                .start_glyph_id
                .checked_add(codepoint - group.start_char_code)?,
        )
        .ok()
    }
}

impl Packed for Format12 {
    type Dep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        let language = rd.read_u32::<BigEndian>()?;
        let num_groups = rd.read_u32::<BigEndian>()?;

        let mut groups = Vec::with_capacity(num_groups as usize);
        for _ in 0..num_groups {
            groups.push(SequentialMapGroup::unpack(&mut rd, ())?);
        }

        Ok(Format12 {
            language,
            num_groups,
            sequential_map_groups: groups,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(self.language)?;
        wr.write_u32::<BigEndian>(self.sequential_map_groups.len() as u32)?;
        for group in &self.sequential_map_groups {
            group.pack(&mut wr, ())?;
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
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        Ok(SequentialMapGroup {
            start_char_code: rd.read_u32::<BigEndian>()?,
            end_char_code: rd.read_u32::<BigEndian>()?,
            start_glyph_id: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
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

    fn get_format4_subtable() -> Format12 {
        let data = include_bytes!("../../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let cmap_record = table.get_table_record("cmap").unwrap();
        let cmap_table: CmapTable = table
            .unpack_required_table("cmap", (), &mut cursor)
            .unwrap();

        let record = cmap_table
            .encoding_records
            .iter()
            .find(|r| r.platform_id == 0 && r.encoding_id == 4)
            .unwrap();

        cursor.set_position((cmap_record.offset + record.offset) as u64);
        let subtable = Subtable::unpack(&mut cursor, ()).unwrap();
        match subtable {
            Subtable::Format12(subtable) => subtable,
            _ => panic!("Expected format 12 subtable"),
        }
    }

    #[test]
    fn test_cmap_subtable_format12_encode_decode() {
        let format12 = get_format4_subtable();

        assert_eq!(
            format12.sequential_map_groups.len(),
            format12.num_groups as usize
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        format12.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            Format12::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            format12
        );
    }

    #[test]
    fn test_cmap_subtable_format12_codepoint_to_glyph_id() {
        let format12 = get_format4_subtable();

        assert_eq!(format12.glyph_id(12), None);
        assert_eq!(format12.glyph_id(13), Some(2));
        assert_eq!(format12.glyph_id(422), Some(360));
        assert_eq!(format12.glyph_id(8694), None);
        assert_eq!(format12.glyph_id(129989), Some(3557));
        assert_eq!(format12.glyph_id(130041), Some(3572));
        assert_eq!(format12.glyph_id(130042), None);
    }
}
