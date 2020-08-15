use std::io::{self, Cursor, Read};

use crate::tables::{FontData, FontTable};
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table contains a dictionary of all font tables included in the file.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/otff
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6.html
#[derive(Debug, PartialEq)]
pub struct OffsetTable {
    /// OpenType fonts that contain TrueType outlines should use the value of 0x00010000. OpenType
    /// fonts containing CFF data (version 1 or 2) should use 0x4F54544F ('OTTO', when
    /// re-interpreted as a Tag).
    pub(crate) sfnt_version: SfntVersion,
    /// Number of tables.
    pub(crate) num_tables: u16,
    /// (Maximum power of 2 <= numTables) x 16.
    pub(crate) search_range: u16,
    /// Log2(maximum power of 2 <= numTables).
    pub(crate) entry_selector: u16,
    /// NumTables x 16-searchRange.
    pub(crate) range_shift: u16,
    /// Table records of the front. Expected to be ordered ascending by their tag.
    pub(crate) tables: Vec<TableRecord>,
}

impl OffsetTable {
    pub fn get_table_record(&self, tag: &str) -> Option<&TableRecord> {
        self.tables
            .binary_search_by(|r| r.tag.as_str().cmp(tag))
            .ok()
            .and_then(|i| self.tables.get(i))
    }

    pub fn unpack_table<'a, T, R, U, S>(
        &self,
        dep: U,
        cursor: &mut Cursor<R>,
    ) -> Result<Option<T>, io::Error>
    where
        R: io::Read + AsRef<[u8]>,
        T: FontTable<'a, U, S>,
    {
        // TODO: return Option for non-required tables?
        let record = match self.get_table_record(T::name()) {
            Some(record) => record,
            None => return Ok(None),
        };

        cursor.set_position(record.offset as u64);
        let mut limit_read = Cursor::new(LimitRead::from_cursor(cursor, record.length as usize));
        Ok(Some(T::unpack(&mut limit_read, dep)?))
    }

    pub fn unpack_required_table<'a, T, R, U, S>(
        &self,
        dep: U,
        cursor: &mut Cursor<R>,
    ) -> Result<T, io::Error>
    where
        R: io::Read + AsRef<[u8]>,
        T: FontTable<'a, U, S>,
    {
        self.unpack_table::<T, R, U, S>(dep, cursor)?
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, format!("{} table missing", T::name()))
            })
    }
}

impl<'a> FontData<'a> for OffsetTable {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let sfnt_version = SfntVersion::unpack(&mut rd, ())?;
        let num_tables = rd.read_u16::<BigEndian>()?;
        let search_range = rd.read_u16::<BigEndian>()?;
        let entry_selector = rd.read_u16::<BigEndian>()?;
        let range_shift = rd.read_u16::<BigEndian>()?;

        let mut tables = Vec::with_capacity(num_tables as usize);
        for _ in 0..num_tables {
            tables.push(TableRecord::unpack(&mut rd, ())?);
        }

        Ok(OffsetTable {
            sfnt_version,
            num_tables,
            search_range,
            entry_selector,
            range_shift,
            tables,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        self.sfnt_version.pack(&mut wr)?;
        wr.write_u16::<BigEndian>(self.num_tables)?;
        wr.write_u16::<BigEndian>(self.search_range)?;
        wr.write_u16::<BigEndian>(self.entry_selector)?;
        wr.write_u16::<BigEndian>(self.range_shift)?;
        for table in &self.tables {
            table.pack(&mut wr)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SfntVersion {
    TrueType,
    CFF,
}

impl<'a> FontData<'a> for SfntVersion {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        match rd.read_u32::<BigEndian>()? {
            0x00010000 => Ok(SfntVersion::TrueType),
            0x4F54544F => Ok(SfntVersion::CFF),
            v => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Unknown sfnt_version {}", v),
            )),
        }
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(match self {
            SfntVersion::TrueType => 0x00010000,
            SfntVersion::CFF => 0x4F54544F,
        })?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct TableRecord {
    pub(crate) tag: String,
    pub(crate) check_sum: u32,
    pub(crate) offset: u32,
    pub(crate) length: u32,
}

impl<'a> FontData<'a> for TableRecord {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let mut tag = [0; 4];
        rd.read_exact(&mut tag)?;
        Ok(TableRecord {
            tag: String::from_utf8_lossy(&tag).to_string(),
            check_sum: rd.read_u32::<BigEndian>()?,
            offset: rd.read_u32::<BigEndian>()?,
            length: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_all(&self.tag.as_bytes())?;
        wr.write_u32::<BigEndian>(self.check_sum)?;
        wr.write_u32::<BigEndian>(self.offset)?;
        wr.write_u32::<BigEndian>(self.length)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_offset_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let table = OffsetTable::unpack(&mut Cursor::new(&data[..]), ()).unwrap();
        assert_eq!(table.sfnt_version, SfntVersion::TrueType);
        assert_eq!(table.num_tables, 17);
        assert_eq!(table.search_range, 256);
        assert_eq!(table.entry_selector, 4);
        assert_eq!(table.range_shift, 16);

        // should include at least the minimal necessary tables for a font to function correctly
        assert!(
            table.tables.iter().any(|t| t.tag == "OS/2"),
            "OS/2 table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "cmap"),
            "cmap table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "glyf"),
            "glyf table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "head"),
            "head table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "hhea"),
            "hhea table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "hmtx"),
            "hmtx table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "loca"),
            "loca table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "maxp"),
            "maxp table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "name"),
            "name table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "post"),
            "post table missing"
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        table.pack(&mut buffer).unwrap();
        assert_eq!(
            OffsetTable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            table
        );
    }
}
