mod format12;
mod format4;

use std::io;

use crate::packed::Packed;
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use format12::Format12;
use format4::Format4;

// TODO: support subtable 14?

/// A font's CMAP table, which ddefines the mapping of character codes to the glyph index values
/// used in the font. Supported character encodings are (in the order of how they are used if they
/// are defined):
/// | platform ID | encoding ID |                                          |
/// |-------------|-------------|------------------------------------------|
/// | 0           | 4           | Unicode >= 2.0, non-BMP allowed          |
/// | 3           | 10          | Windows, full Unicode                    |
/// | 0           | 3           | Unicode >= 2.0, BMP only                 |
/// | 3           | 1           | Windows, compatbility with older devices |
///
/// Supported subtable formats are: 4 and 12
///
/// Not supported character encodings and subtable formats are ignored. An error is returned, if
/// there is not a single supported character encoding and subtable combination.
///
/// See OpenType sepc: https://docs.microsoft.com/en-us/typography/opentype/spec/cmap
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
    /// Byte offset from beginning of table to the subtable for this encoding.
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

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        let mut buf = Vec::new();
        match self {
            Subtable::Format4(subtable) => subtable.pack(&mut buf)?,
            Subtable::Format12(subtable) => subtable.pack(&mut buf)?,
        }

        if buf.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("CMAP subtable cannot be bigger than {} bytes", u16::MAX),
            ));
        }

        wr.write_u16::<BigEndian>(match self {
            Subtable::Format4(_) => 4,
            Subtable::Format12(_) => 12,
        })?;
        wr.write_u16::<BigEndian>(buf.len() as u16)?;
        wr.write_all(&buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_cmap_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor).unwrap();
        let cmap_table: CmapTable = table.unpack_required_table("cmap", &mut cursor).unwrap();

        assert_eq!(cmap_table.version, 0);
        assert_eq!(cmap_table.num_tables, 4);
        assert_eq!(
            cmap_table.encoding_records,
            vec![
                EncodingRecord {
                    platform_id: 0,
                    encoding_id: 3,
                    offset: 36,
                },
                EncodingRecord {
                    platform_id: 0,
                    encoding_id: 4,
                    offset: 1740,
                },
                EncodingRecord {
                    platform_id: 3,
                    encoding_id: 1,
                    offset: 36,
                },
                EncodingRecord {
                    platform_id: 3,
                    encoding_id: 10,
                    offset: 1740,
                },
            ]
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        cmap_table.pack(&mut buffer).unwrap();
        assert_eq!(
            CmapTable::unpack(&mut Cursor::new(buffer)).unwrap(),
            cmap_table
        );
    }
}
