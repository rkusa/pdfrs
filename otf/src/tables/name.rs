use std::io;

use super::FontTable;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table includes human-readable names for features and settings, copyright notices,
/// font names, style names, and other information related to the font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/name
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6name.html
#[derive(Debug, PartialEq)]
pub enum NameTable {
    Format0(Format0NameTable),
    Format1(Format1NameTable),
}

impl<'a> FontTable<'a> for NameTable {
    type Dep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        let format = rd.read_u16::<BigEndian>()?;
        match format {
            0 => Ok(NameTable::Format0(Format0NameTable::unpack(&mut rd, ())?)),
            1 => Ok(NameTable::Format1(Format1NameTable::unpack(&mut rd, ())?)),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Invalid NAME table format {}", format),
            )),
        }
    }

    fn pack<W: io::Write>(&'a self, mut wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        match self {
            NameTable::Format0(table) => {
                // format
                wr.write_u16::<BigEndian>(0)?;
                table.pack(&mut wr, ())?;
            }
            NameTable::Format1(table) => {
                // format
                wr.write_u16::<BigEndian>(1)?;
                table.pack(&mut wr, ())?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Format0NameTable {
    /// Number of name records.
    count: u16,
    /// Offset to start of string storage (from start of table).
    offset: u16,
    /// The name records.
    name_records: Vec<NameRecord>,
    /// Raw storage area for the actual UTF-16BE encoded string data.
    string_data: Vec<u8>,
}

impl<'a> FontTable<'a> for Format0NameTable {
    type Dep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        let count = rd.read_u16::<BigEndian>()?;
        let offset = rd.read_u16::<BigEndian>()?;
        let mut name_records = Vec::with_capacity(count as usize);
        for _ in 0..count {
            name_records.push(NameRecord::unpack(&mut rd, ())?);
        }
        let mut string_data = Vec::new(); // TODO: guess capacity?
        rd.read_to_end(&mut string_data)?;
        Ok(Format0NameTable {
            count,
            offset,
            name_records,
            string_data,
        })
    }

    fn pack<W: io::Write>(&'a self, mut wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        // TODO: update count, offset and string_data based on name_records
        wr.write_u16::<BigEndian>(self.count)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        for record in &self.name_records {
            record.pack(&mut wr, ())?;
        }
        wr.write_all(&self.string_data)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Format1NameTable {
    /// Number of name records.
    count: u16,
    /// Offset to start of string storage (from start of table).
    offset: u16,
    /// The name records.
    name_records: Vec<NameRecord>,
    /// Number of language-tag records.
    lang_tag_count: u16,
    /// The language-tag records.
    lang_tag_records: Vec<LangTagRecord>,
    /// Raw storage area for the actual UTF-16BE encoded string data.
    string_data: Vec<u8>,
}

impl<'a> FontTable<'a> for Format1NameTable {
    type Dep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        let count = rd.read_u16::<BigEndian>()?;
        let offset = rd.read_u16::<BigEndian>()?;
        let mut name_records = Vec::with_capacity(count as usize);
        for _ in 0..count {
            name_records.push(NameRecord::unpack(&mut rd, ())?);
        }

        let lang_tag_count = rd.read_u16::<BigEndian>()?;
        let mut lang_tag_records = Vec::with_capacity(lang_tag_count as usize);
        for _ in 0..lang_tag_count {
            lang_tag_records.push(LangTagRecord::unpack(&mut rd, ())?);
        }
        let mut string_data = Vec::new(); // TODO: guess capacity?
        rd.read_to_end(&mut string_data)?;
        Ok(Format1NameTable {
            count,
            offset,
            name_records,
            lang_tag_count,
            lang_tag_records,
            string_data,
        })
    }

    fn pack<W: io::Write>(&'a self, mut wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        // TODO: update count, offset and string_data based on name_records (same for lang tags)
        wr.write_u16::<BigEndian>(self.count)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        for record in &self.name_records {
            record.pack(&mut wr, ())?;
        }
        wr.write_u16::<BigEndian>(self.lang_tag_count)?;
        for record in &self.lang_tag_records {
            record.pack(&mut wr, ())?;
        }
        wr.write_all(&self.string_data)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct NameRecord {
    /// Platform ID,
    platform_id: u16,
    /// Platform-specific encoding ID.
    encoding_id: u16,
    /// Language ID.
    language_id: u16,
    /// Name ID.
    name_id: u16,
    /// String length (in bytes).
    length: u16,
    /// String offset from start of storage area (in bytes).
    offset: u16,
}

impl<'a> FontTable<'a> for NameRecord {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        Ok(NameRecord {
            platform_id: rd.read_u16::<BigEndian>()?,
            encoding_id: rd.read_u16::<BigEndian>()?,
            language_id: rd.read_u16::<BigEndian>()?,
            name_id: rd.read_u16::<BigEndian>()?,
            length: rd.read_u16::<BigEndian>()?,
            offset: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.platform_id)?;
        wr.write_u16::<BigEndian>(self.encoding_id)?;
        wr.write_u16::<BigEndian>(self.language_id)?;
        wr.write_u16::<BigEndian>(self.name_id)?;
        wr.write_u16::<BigEndian>(self.length)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct LangTagRecord {
    /// Language-tag string length (in bytes)
    length: u16,
    /// Language-tag string offset from start of storage area (in bytes).
    offset: u16,
}

impl<'a> FontTable<'a> for LangTagRecord {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        Ok(LangTagRecord {
            length: rd.read_u16::<BigEndian>()?,
            offset: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.length)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_name_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let name_table: NameTable = table
            .unpack_required_table("name", (), &mut cursor)
            .unwrap();

        match &name_table {
            NameTable::Format0(format0) => {
                assert_eq!(format0.name_records.len(), format0.count as usize);
            }
            NameTable::Format1(_) => panic!("Expected name table format 0"),
        }

        // re-pack and compare
        let mut buffer = Vec::new();
        name_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            NameTable::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            name_table
        );
    }
}
