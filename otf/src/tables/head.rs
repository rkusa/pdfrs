use std::io;

use crate::packed::Packed;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// See https://docs.microsoft.com/en-us/typography/opentype/spec/head
#[derive(Debug, PartialEq)]
pub struct HeadTable {
    major_version: u16,
    minor_version: u16,
    font_revision: (i16, u16),
    check_sum_adjustment: u32,
    magic_number: u32,
    flags: u16,
    units_per_em: u16,
    created: i64,
    modified: i64,
    x_min: i16,
    y_min: i16,
    x_max: i16,
    y_max: i16,
    mac_style: u16,
    lowest_rec_ppem: u16,
    font_direction_hint: i16,
    index_to_loc_format: i16,
    glyph_data_format: i16,
}

impl Packed for HeadTable {
    fn unpack<R: io::Read>(rd: &mut R) -> Result<Self, io::Error> {
        let major_version = rd.read_u16::<BigEndian>()?;
        let minor_version = rd.read_u16::<BigEndian>()?;

        let decimal = rd.read_i16::<BigEndian>()?;
        let fraction = rd.read_u16::<BigEndian>()?;

        // const decimal = dataView.getInt16(offset, false);
        // const fraction = dataView.getUint16(offset + 2, false);
        // return decimal + fraction / 65535;
        Ok(HeadTable {
            major_version,
            minor_version,
            font_revision: (decimal, fraction),
            check_sum_adjustment: rd.read_u32::<BigEndian>()?,
            magic_number: rd.read_u32::<BigEndian>()?,
            flags: rd.read_u16::<BigEndian>()?,
            units_per_em: rd.read_u16::<BigEndian>()?,
            created: rd.read_i64::<BigEndian>()?,
            modified: rd.read_i64::<BigEndian>()?,
            x_min: rd.read_i16::<BigEndian>()?,
            y_min: rd.read_i16::<BigEndian>()?,
            x_max: rd.read_i16::<BigEndian>()?,
            y_max: rd.read_i16::<BigEndian>()?,
            mac_style: rd.read_u16::<BigEndian>()?,
            lowest_rec_ppem: rd.read_u16::<BigEndian>()?,
            font_direction_hint: rd.read_i16::<BigEndian>()?,
            index_to_loc_format: rd.read_i16::<BigEndian>()?,
            glyph_data_format: rd.read_i16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.major_version)?;
        wr.write_u16::<BigEndian>(self.minor_version)?;
        wr.write_i16::<BigEndian>(self.font_revision.0)?;
        wr.write_u16::<BigEndian>(self.font_revision.1)?;
        wr.write_u32::<BigEndian>(self.check_sum_adjustment)?;
        wr.write_u32::<BigEndian>(self.magic_number)?;
        wr.write_u16::<BigEndian>(self.flags)?;
        wr.write_u16::<BigEndian>(self.units_per_em)?;
        wr.write_i64::<BigEndian>(self.created)?;
        wr.write_i64::<BigEndian>(self.modified)?;
        wr.write_i16::<BigEndian>(self.x_min)?;
        wr.write_i16::<BigEndian>(self.y_min)?;
        wr.write_i16::<BigEndian>(self.x_max)?;
        wr.write_i16::<BigEndian>(self.y_max)?;
        wr.write_u16::<BigEndian>(self.mac_style)?;
        wr.write_u16::<BigEndian>(self.lowest_rec_ppem)?;
        wr.write_i16::<BigEndian>(self.font_direction_hint)?;
        wr.write_i16::<BigEndian>(self.index_to_loc_format)?;
        wr.write_i16::<BigEndian>(self.glyph_data_format)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn head_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf");
        let mut cursor = Cursor::new(data.to_vec());
        let table = OffsetTable::unpack(&mut cursor).unwrap();
        let head_record = table.get_table_record("head").unwrap();

        cursor.set_position(head_record.offset as u64);
        let head = HeadTable::unpack(&mut cursor).unwrap();

        assert_eq!(head.major_version, 1);
        assert_eq!(head.minor_version, 0);
        // font_revision = 3.031
        assert_eq!(head.font_revision.0, 3);
        assert!((head.font_revision.1 as f32 / 65535.0 - 0.031).abs() < 0.00001);
        assert_eq!(head.check_sum_adjustment, 3547005195);
        assert_eq!(head.magic_number, 1594834165);
        assert_eq!(head.flags, 13);
        assert_eq!(head.units_per_em, 1000);
        assert_eq!(head.created, 3562553439);
        assert_eq!(head.modified, 3678044538);
        assert_eq!(head.x_min, -1000);
        assert_eq!(head.y_min, -505);
        assert_eq!(head.x_max, 1134);
        assert_eq!(head.y_max, 1188);
        assert_eq!(head.mac_style, 0);
        assert_eq!(head.lowest_rec_ppem, 8);
        assert_eq!(head.font_direction_hint, 0);
        assert_eq!(head.index_to_loc_format, 1);
        assert_eq!(head.glyph_data_format, 0);

        // re-pack and compare
        let mut buffer = Vec::new();
        head.pack(&mut buffer).unwrap();
        assert_eq!(HeadTable::unpack(&mut Cursor::new(buffer)).unwrap(), head);
    }
}
