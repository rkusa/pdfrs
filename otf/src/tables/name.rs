use std::io::{self, Cursor};

use super::{FontData, FontTable};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table includes human-readable names for features and settings, copyright notices,
/// font names, style names, and other information related to the font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/name
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6name.html
#[derive(Debug, PartialEq, Clone)]
pub enum NameTable {
    Format0(Format0NameTable),
    Format1(Format1NameTable),
}

impl NameTable {
    pub(crate) fn font_family_name(&self) -> Option<String> {
        match self {
            NameTable::Format0(table) => table.font_family_name(),
            NameTable::Format1(table) => table.font_family_name(),
        }
    }

    pub(crate) fn post_script_name(&self) -> Option<String> {
        match self {
            NameTable::Format0(table) => table.post_script_name(),
            NameTable::Format1(table) => table.post_script_name(),
        }
    }
}

impl<'a> FontTable<'a, (), (), ()> for NameTable {
    fn name() -> &'static str {
        "name"
    }
}

impl<'a> FontData<'a> for NameTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
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

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
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

#[derive(Debug, PartialEq, Clone)]
pub struct Format0NameTable {
    /// Number of name records.
    count: u16,
    /// Offset to start of string storage (from start of table).
    offset: u16,
    /// The name records.
    name_records: Vec<NameRecord>,
    /// Raw storage area for the actual UTF-16BE encoded string data.
    string_data: Vec<u16>,
}

impl Format0NameTable {
    pub(crate) fn font_family_name(&self) -> Option<String> {
        // Only searching for Windows/Unicode for now
        // TODO: add support for other platform/encodings
        let name_record = self
            .name_records
            .iter()
            .find(|r| r.platform_id == 3 && r.encoding_id == 1 && r.name_id == 1)?;
        let start = (name_record.offset / 2) as usize;
        let end = start + (name_record.length / 2) as usize;
        String::from_utf16(&self.string_data[start..end]).ok()
    }

    pub(crate) fn post_script_name(&self) -> Option<String> {
        // Only searching for Windows/Unicode for now
        // TODO: add support for other platform/encodings
        let name_record = self
            .name_records
            .iter()
            .find(|r| r.platform_id == 3 && r.encoding_id == 1 && r.name_id == 6)?;
        let start = (name_record.offset / 2) as usize;
        let end = start + (name_record.length / 2) as usize;
        String::from_utf16(&self.string_data[start..end]).ok()
    }
}

impl<'a> FontData<'a> for Format0NameTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let count = rd.read_u16::<BigEndian>()?;
        let offset = rd.read_u16::<BigEndian>()?;
        let mut name_records = Vec::with_capacity(count as usize);
        for _ in 0..count {
            name_records.push(NameRecord::unpack(&mut rd, ())?);
        }
        let mut string_data = Vec::new(); // TODO: guess capacity?
        loop {
            match rd.read_u16::<BigEndian>() {
                Ok(n) => string_data.push(n),
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err),
            }
        }
        Ok(Format0NameTable {
            count,
            offset,
            name_records,
            string_data,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        // TODO: update count, offset and string_data based on name_records
        wr.write_u16::<BigEndian>(self.count)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        for record in &self.name_records {
            record.pack(&mut wr, ())?;
        }
        for n in &self.string_data {
            wr.write_u16::<BigEndian>(*n)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
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
    string_data: Vec<u16>,
}

impl Format1NameTable {
    pub(crate) fn font_family_name(&self) -> Option<String> {
        // Only searching for Windows/Unicode for now
        // TODO: add support for other platform/encodings
        let name_record = self
            .name_records
            .iter()
            .find(|r| r.platform_id == 3 && r.encoding_id == 1 && r.name_id == 1)?;
        let start = (name_record.offset / 2) as usize;
        let end = start + (name_record.length / 2) as usize;
        String::from_utf16(&self.string_data[start..end]).ok()
    }

    pub(crate) fn post_script_name(&self) -> Option<String> {
        // Only searching for Windows/Unicode for now
        // TODO: add support for other platform/encodings
        let name_record = self
            .name_records
            .iter()
            .find(|r| r.platform_id == 3 && r.encoding_id == 1 && r.name_id == 6)?;
        let start = (name_record.offset / 2) as usize;
        let end = start + (name_record.length / 2) as usize;
        String::from_utf16(&self.string_data[start..end]).ok()
    }
}

impl<'a> FontData<'a> for Format1NameTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
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
        loop {
            match rd.read_u16::<BigEndian>() {
                Ok(n) => string_data.push(n),
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err),
            }
        }
        Ok(Format1NameTable {
            count,
            offset,
            name_records,
            lang_tag_count,
            lang_tag_records,
            string_data,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
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
        for n in &self.string_data {
            wr.write_u16::<BigEndian>(*n)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
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

impl<'a> FontData<'a> for NameRecord {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        Ok(NameRecord {
            platform_id: rd.read_u16::<BigEndian>()?,
            encoding_id: rd.read_u16::<BigEndian>()?,
            language_id: rd.read_u16::<BigEndian>()?,
            name_id: rd.read_u16::<BigEndian>()?,
            length: rd.read_u16::<BigEndian>()?,
            offset: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.platform_id)?;
        wr.write_u16::<BigEndian>(self.encoding_id)?;
        wr.write_u16::<BigEndian>(self.language_id)?;
        wr.write_u16::<BigEndian>(self.name_id)?;
        wr.write_u16::<BigEndian>(self.length)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct LangTagRecord {
    /// Language-tag string length (in bytes)
    length: u16,
    /// Language-tag string offset from start of storage area (in bytes).
    offset: u16,
}

impl<'a> FontData<'a> for LangTagRecord {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        Ok(LangTagRecord {
            length: rd.read_u16::<BigEndian>()?,
            offset: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.length)?;
        wr.write_u16::<BigEndian>(self.offset)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::OffsetTable;

    fn get_name_table() -> NameTable {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let name_table: NameTable = table.unpack_required_table((), &mut cursor).unwrap();

        name_table
    }

    fn get_format0(name_table: &NameTable) -> &Format0NameTable {
        match &name_table {
            NameTable::Format0(format0) => format0,
            NameTable::Format1(_) => panic!("Expected name table format 0"),
        }
    }

    #[test]
    fn test_name_table_encode_decode() {
        let name_table = get_name_table();
        let format0 = get_format0(&name_table);
        assert_eq!(format0.name_records.len(), format0.count as usize);

        // re-pack and compare
        let mut buffer = Vec::new();
        name_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            NameTable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            name_table
        );
    }

    #[test]
    fn test_name_table_font_family_name() {
        let name_table = get_name_table();
        assert_eq!(name_table.font_family_name().as_deref(), Some("Iosevka"));
    }

    #[test]
    fn test_name_table_post_script_name() {
        let name_table = get_name_table();
        assert_eq!(name_table.post_script_name().as_deref(), Some("Iosevka"));
    }
}
