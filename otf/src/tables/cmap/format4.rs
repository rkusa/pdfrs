use std::io;

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
    id_range_offset: Vec<u16>,
    glyph_id_array: Vec<u16>,
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
        let mut id_range_offset = vec![0; seg_count];
        rd.read_u16_into::<BigEndian>(&mut id_range_offset)?;

        // TODO: guess a capacity
        let mut glyph_id_array = Vec::new();
        loop {
            match rd.read_u16::<BigEndian>() {
                Ok(id) => glyph_id_array.push(id),
                // read ids until there is nothing more to read
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err),
            }
        }

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
            id_range_offset,
            glyph_id_array,
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
        for id_range_offset in &self.id_range_offset {
            wr.write_u16::<BigEndian>(*id_range_offset)?;
        }
        for glyph_id in &self.glyph_id_array {
            wr.write_u16::<BigEndian>(*glyph_id)?;
        }
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
    fn test_cmap_subtable_format4_encode_decode() {
        let data = include_bytes!("../../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor).unwrap();
        let cmap_record = table.get_table_record("cmap").unwrap();
        let cmap_table: CmapTable = table.unpack_required_table("cmap", &mut cursor).unwrap();

        let record = cmap_table
            .encoding_records
            .iter()
            .find(|r| r.platform_id == 3 && r.encoding_id == 1)
            .unwrap();

        dbg!(&record);

        cursor.set_position((cmap_record.offset + record.offset) as u64);
        let subtable = Subtable::unpack(&mut cursor).unwrap();
        let format4 = match subtable {
            Subtable::Format4(subtable) => subtable,
            _ => panic!("Expected format 4 subtable"),
        };

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
}
