mod format12;
mod format4;

use std::io;

use crate::packed::Packed;
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use format12::Format12;
use format4::Format4;

// Notes: supported tables:
// platform_id, encoding_id, format
// 0, 4 -> Unicode >= 2.0 non-BMP allowed
// 0, 3, 4 -> Unicode >= 2.0 BMP only
// 3, 10, 12 -> Windows, full Unicode
// 3, 1, 4 -> Windows, compatbility with older devices
// Supported formats: 4, 12
// Later: 14

/// See https://docs.microsoft.com/en-us/typography/opentype/spec/cmap
#[derive(Debug, PartialEq)]
pub struct CmapTable {
    version: u16,
    num_tables: u16,
    encoding_records: Vec<EncodingRecord>,
}

impl Packed for CmapTable {
    fn unpack<R: io::Read>(mut rd: &mut R) -> Result<Self, io::Error> {
        let version = rd.read_u16::<BigEndian>()?;
        let num_tables = rd.read_u16::<BigEndian>()?;

        let mut encoding_records = Vec::with_capacity(num_tables.min(4) as usize);
        for _ in 0..num_tables {
            let record = EncodingRecord::unpack(&mut rd)?;
            // skip unsupported formats
            if !matches!(
                (record.platform_id, record.encoding_id),
                (0, 4) | (0, 3) | (3, 10) | (3, 1)
            ) {
                continue;
            }
            encoding_records.push(record);
        }

        if encoding_records.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Font does not contain any supported CMAP",
            ));
        }

        Ok(CmapTable {
            version,
            num_tables,
            encoding_records,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.version)?;
        wr.write_u16::<BigEndian>(self.num_tables)?;
        for table in &self.encoding_records {
            table.pack(&mut wr)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct EncodingRecord {
    platform_id: u16,
    encoding_id: u16,
    offset: u32,
}

impl Packed for EncodingRecord {
    fn unpack<R: io::Read>(rd: &mut R) -> Result<Self, io::Error> {
        Ok(EncodingRecord {
            platform_id: rd.read_u16::<BigEndian>()?,
            encoding_id: rd.read_u16::<BigEndian>()?,
            offset: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.platform_id)?;
        wr.write_u16::<BigEndian>(self.encoding_id)?;
        wr.write_u32::<BigEndian>(self.offset)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum Subtable {
    Format4(Format4),
    Format12(Format12),
}

impl Packed for Subtable {
    fn unpack<R: io::Read>(rd: &mut R) -> Result<Self, io::Error> {
        let format = rd.read_u16::<BigEndian>()?;
        let length = rd.read_u16::<BigEndian>()?;

        let mut rd = LimitRead::new(rd, length as usize);

        match format {
            4 => Ok(Subtable::Format4(Format4::unpack(&mut rd)?)),
            12 => Ok(Subtable::Format12(Format12::unpack(&mut rd)?)),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("CMAP subtable format {} is not supported", format),
            )),
        }
    }

    fn pack<W: io::Write>(&self, _wr: &mut W) -> Result<(), io::Error> {
        unimplemented!()
    }
}

#[cfg(test)]
mod test {
    use std::io::{Cursor, Read};

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn cmap_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf");
        let mut cursor = Cursor::new(data.to_vec());
        let table = OffsetTable::unpack(&mut cursor).unwrap();
        let head_record = table.get_table_record("cmap").unwrap();

        cursor.set_position(head_record.offset as u64);
        let head = CmapTable::unpack(&mut cursor).unwrap();

        assert_eq!(head.version, 0);
        assert_eq!(head.num_tables, 4);

        // re-pack and compare
        // let mut buffer = Vec::new();
        // head.pack(&mut buffer).unwrap();
        // assert_eq!(CmapTable::unpack(Cursor::new(buffer)).unwrap(), head);
    }

    #[test]
    fn limit_read() {
        let data = "foobar".as_bytes().to_vec();
        let mut rd = LimitRead::new(Cursor::new(data), 5);

        let mut buf = [0; 2];
        assert_eq!((rd.read(&mut buf).unwrap(), &buf), (2, b"fo"));
        assert_eq!((rd.read(&mut buf).unwrap(), &buf), (2, b"ob"));
        assert_eq!((rd.read(&mut buf).unwrap(), &buf[..1]), (1, &b"a"[..]));
        assert_eq!(rd.read(&mut buf).unwrap(), 0);
    }
}
