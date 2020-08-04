use std::convert::TryFrom;
use std::io;
use std::mem;

use crate::packed::Packed;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, PartialEq)]
pub struct Format4 {
    language: u16,
    seg_count_x2: u16,
    search_range: u16,
    entry_selector: u16,
    range_shift: u16,
    end_code: Vec<u16>,
    reserved_pad: u16,
    start_code: Vec<u16>,
    id_delta: Vec<i16>,
    /// Raw byte data of both `id_range_offset` and `glyph_id_array`
    id_data: Vec<u8>,
    id_range_offset: Vec<u16>,
    // glyph_id_array not needed as a seperate vec
    // glyph_id_array: Vec<u16>,
}

impl Format4 {
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        // Return None for codepoints > `u16::MAX`
        let codepoint = u16::try_from(codepoint).ok()?;

        // Search for the first end_code that is greater than or equal to the character code
        let ix = match self.end_code.binary_search(&codepoint) {
            // Found a direct match
            Ok(ix) => ix,
            // No direct match, `ix` represents the position where the codepoint could be inserted
            // while maintaining sorted order -> the index represents the first end code that is
            // greater than the character code
            Err(ix) => ix,
        };
        let start_code = *self.start_code.get(ix)?;

        if start_code <= codepoint {
            let id_range_offset = *self.id_range_offset.get(ix)?;
            let id_delta = *self.id_delta.get(ix)?;
            let val = if id_range_offset == 0 {
                codepoint
            } else {
                // id_range_offset + (codepoint - start_code) * 2 + 2 * ix
                let pos = id_range_offset
                    .checked_add(codepoint.checked_sub(start_code)?.checked_mul(2)?)?
                    .checked_add(
                        (mem::size_of::<u16>() as u16).checked_mul(u16::try_from(ix).ok()?)?,
                    )? as usize;

                (&self.id_data[pos..])
                    .read_u16::<BigEndian>()
                    .ok()
                    .filter(|v| *v != 0)?
            };
            Some(u16::try_from((val as i32) + id_delta as i32).ok()?)
        } else {
            None
        }
    }
}

impl Packed for Format4 {
    fn unpack<R: io::Read>(rd: &mut R) -> Result<Self, io::Error> {
        let language = rd.read_u16::<BigEndian>()?;
        let seg_count_x2 = rd.read_u16::<BigEndian>()?;
        let seg_count = (seg_count_x2 / 2) as usize;
        let search_range = rd.read_u16::<BigEndian>()?;
        let entry_selector = rd.read_u16::<BigEndian>()?;
        let range_shift = rd.read_u16::<BigEndian>()?;
        let mut end_code = vec![0; seg_count];
        rd.read_u16_into::<BigEndian>(&mut end_code)?;
        let reserved_pad = rd.read_u16::<BigEndian>()?;
        let mut start_code = vec![0; seg_count];
        rd.read_u16_into::<BigEndian>(&mut start_code)?;
        let mut id_delta = vec![0; seg_count];
        rd.read_i16_into::<BigEndian>(&mut id_delta)?;

        let mut id_data = Vec::new();
        rd.read_to_end(&mut id_data)?;

        let mut id_range_offset = vec![0; seg_count];
        (&id_data[..]).read_u16_into::<BigEndian>(&mut id_range_offset)?;

        Ok(Format4 {
            language,
            seg_count_x2,
            search_range,
            entry_selector,
            range_shift,
            end_code,
            reserved_pad,
            start_code,
            id_delta,
            id_data,
            id_range_offset,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.language)?;
        wr.write_u16::<BigEndian>(self.seg_count_x2)?;
        let search_range = 2 * 2u16.pow(((self.seg_count_x2 / 2) as f32).log2().floor() as u32);
        wr.write_u16::<BigEndian>(search_range)?;
        wr.write_u16::<BigEndian>((search_range as f32 / 2.0).log2() as u16)?; // entry_selector
        wr.write_u16::<BigEndian>(self.seg_count_x2 - search_range)?; // range_shift
        for end_code in &self.end_code {
            wr.write_u16::<BigEndian>(*end_code)?;
        }
        wr.write_u16::<BigEndian>(0)?;
        for start_code in &self.start_code {
            wr.write_u16::<BigEndian>(*start_code)?;
        }
        for id_delta in &self.id_delta {
            wr.write_i16::<BigEndian>(*id_delta)?;
        }
        // `id_range_offset` and `glyph_id_array` are part of id_data
        wr.write_all(&self.id_data)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::tables::cmap::{CmapTable, Subtable};
    use crate::OffsetTable;

    fn get_format4_subtable() -> Format4 {
        let data = include_bytes!("../../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor).unwrap();
        let cmap_record = table.get_table_record("cmap").unwrap();
        let cmap_table: CmapTable = table.unpack_required_table("cmap", &mut cursor).unwrap();

        let record = cmap_table
            .encoding_records
            .iter()
            .find(|r| r.platform_id == 0 && r.encoding_id == 3)
            .unwrap();

        cursor.set_position((cmap_record.offset + record.offset) as u64);
        let subtable = Subtable::unpack(&mut cursor).unwrap();
        match subtable {
            Subtable::Format4(subtable) => subtable,
            _ => panic!("Expected format 4 subtable"),
        }
    }

    #[test]
    fn test_cmap_subtable_format4_encode_decode() {
        let format4 = get_format4_subtable();

        let seg_count = format4.seg_count_x2 as usize / 2;
        assert_eq!(format4.end_code.len(), seg_count);
        assert_eq!(format4.start_code.len(), seg_count);
        assert_eq!(format4.id_delta.len(), seg_count);
        assert_eq!(format4.id_range_offset.len(), seg_count);

        // re-pack and compare
        let mut buffer = Vec::new();
        format4.pack(&mut buffer).unwrap();
        assert_eq!(Format4::unpack(&mut Cursor::new(buffer)).unwrap(), format4);
    }

    #[test]
    fn test_cmap_subtable_format4_codepoint_to_glyph_id_without_range_offset() {
        let format4 = Format4 {
            language: 0,
            seg_count_x2: 8,
            search_range: 8,
            entry_selector: 4,
            range_shift: 0,
            end_code: vec![20, 90, 480, 0xFFFF],
            reserved_pad: 0,
            start_code: vec![10, 30, 153, 0xFFFF],
            id_delta: vec![-9, -18, -27, 1],
            id_data: vec![0; 8], // 4 segments, each being 0
            id_range_offset: vec![0, 0, 0, 0],
        };

        assert_eq!(format4.glyph_id(10), Some(1));
        assert_eq!(format4.glyph_id(20), Some(11));
        assert_eq!(format4.glyph_id(30), Some(12));
        assert_eq!(format4.glyph_id(90), Some(72));
    }

    #[test]
    fn test_cmap_subtable_format4_codepoint_to_glyph_id_with_range_offset() {
        let format4 = Format4 {
            language: 0,
            seg_count_x2: 4,
            search_range: 4,
            entry_selector: 1,
            range_shift: 0,
            end_code: vec![12, 0xFFFF],
            reserved_pad: 0,
            start_code: vec![10, 0xFFFF],
            id_delta: vec![0, 1],
            id_data: vec![
                0x00, 0x04, // segment 1: 4
                0x00, 0x00, // segment 2: 0
                0x00, 0x01, // glyph_id_array 0: 1
                0x00, 0x02, // glyph_id_array 1: 2
                0x00, 0x03, // glyph_id_array 2: 3
            ],
            id_range_offset: vec![4, 0],
        };

        assert_eq!(format4.glyph_id(0), None);
        assert_eq!(format4.glyph_id(10), Some(1));
        assert_eq!(format4.glyph_id(11), Some(2));
        assert_eq!(format4.glyph_id(12), Some(3));
        assert_eq!(format4.glyph_id(13), None);

        // re-pack and compare
        let mut buffer = Vec::new();
        format4.pack(&mut buffer).unwrap();
        assert_eq!(Format4::unpack(&mut Cursor::new(buffer)).unwrap(), format4);
    }
}
