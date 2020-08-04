mod packed;
mod tables;
mod utils;

use std::io::{self, Cursor};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use packed::Packed;
use utils::limit_read::LimitRead;

// Spec: https://docs.microsoft.com/en-us/typography/opentype/spec/otff

pub struct OpenTypeFont {
    offset_table: OffsetTable,
    cmap_table: tables::cmap::CmapTable,
    head_table: tables::head::HeadTable,
}

impl OpenTypeFont {
    pub fn from_slice(data: impl AsRef<[u8]>) -> Result<Self, io::Error> {
        let mut cursor = Cursor::new(data.as_ref());
        let offset_table = OffsetTable::unpack(&mut cursor)?;

        Ok(OpenTypeFont {
            cmap_table: offset_table.unpack_required_table("cmap", &mut cursor)?,
            head_table: offset_table.unpack_required_table("head", &mut cursor)?,
            offset_table,
        })
    }

    pub fn to_writer(&self, mut wr: impl io::Write) -> Result<(), io::Error> {
        // TODO: update table entry offsets
        self.offset_table.pack(&mut wr)?;

        // TODO: write in correct order
        self.cmap_table.pack(&mut wr)?;
        self.head_table.pack(&mut wr)?;

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
struct OffsetTable {
    sfnt_version: SfntVersion,
    num_tables: u16,
    search_range: u16,
    entry_selector: u16,
    range_shift: u16,
    // expected to be ordered ascending by their tag
    tables: Vec<TableRecord>,
}

impl OffsetTable {
    fn get_table_record(&self, tag: &str) -> Option<&TableRecord> {
        self.tables
            .binary_search_by(|r| r.tag.as_str().cmp(tag))
            .ok()
            .and_then(|i| self.tables.get(i))
    }

    fn unpack_table<T, R>(&self, tag: &str, cursor: &mut Cursor<R>) -> Result<Option<T>, io::Error>
    where
        R: io::Read + AsRef<[u8]>,
        T: Packed,
    {
        // TODO: return Option for non-required tables?
        let record = match self.get_table_record(tag) {
            Some(record) => record,
            None => return Ok(None),
        };
        cursor.set_position(record.offset as u64);
        let mut limit_read = LimitRead::new(cursor, record.length as usize);
        Ok(Some(T::unpack(&mut limit_read)?))
    }

    fn unpack_required_table<T, R>(&self, tag: &str, cursor: &mut Cursor<R>) -> Result<T, io::Error>
    where
        R: io::Read + AsRef<[u8]>,
        T: Packed,
    {
        self.unpack_table::<T, R>(tag, cursor)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, format!("{} table missing", tag)))
    }
}

impl Packed for OffsetTable {
    fn unpack<R: io::Read>(mut rd: &mut R) -> Result<Self, io::Error> {
        let sfnt_version = SfntVersion::unpack(&mut rd)?;
        let num_tables = rd.read_u16::<BigEndian>()?;
        let search_range = rd.read_u16::<BigEndian>()?;
        let entry_selector = rd.read_u16::<BigEndian>()?;
        let range_shift = rd.read_u16::<BigEndian>()?;

        let mut tables = Vec::with_capacity(num_tables as usize);
        for _ in 0..num_tables {
            tables.push(TableRecord::unpack(&mut rd)?);
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

        let x = 2u16.pow((self.num_tables as f32).log2() as u32);
        let search_range = x * 16;
        wr.write_u16::<BigEndian>(search_range)?;
        let entry_selector = (x as f32).log2() as u16;
        wr.write_u16::<BigEndian>(entry_selector)?;
        let range_shift = self.num_tables * 16 - search_range;
        wr.write_u16::<BigEndian>(range_shift)?;
        for table in &self.tables {
            table.pack(&mut wr)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
enum SfntVersion {
    TrueType,
    CFF,
}

impl SfntVersion {
    fn unpack(mut rd: impl io::Read) -> Result<Self, io::Error> {
        match rd.read_u32::<BigEndian>()? {
            0x00010000 => Ok(SfntVersion::TrueType),
            0x4F54544F => Ok(SfntVersion::CFF),
            v => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Unknown sfnt_version {}", v),
            )),
        }
    }

    fn pack(&self, mut wr: impl io::Write) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(match self {
            SfntVersion::TrueType => 0x00010000,
            SfntVersion::CFF => 0x4F54544F,
        })?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
struct TableRecord {
    tag: String,
    check_sum: u32,
    offset: u32,
    length: u32,
}

impl TableRecord {
    fn unpack(mut rd: impl io::Read) -> Result<Self, io::Error> {
        let mut tag = [0; 4];
        rd.read_exact(&mut tag)?;
        Ok(TableRecord {
            tag: String::from_utf8_lossy(&tag).to_string(),
            check_sum: rd.read_u32::<BigEndian>()?,
            offset: rd.read_u32::<BigEndian>()?,
            length: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack(&self, mut wr: impl io::Write) -> Result<(), io::Error> {
        wr.write_all(&self.tag.as_bytes())?;
        wr.write_u32::<BigEndian>(self.check_sum)?;
        wr.write_u32::<BigEndian>(self.offset)?;
        wr.write_u32::<BigEndian>(self.length)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_offset_table_encode_decode() {
        let data = include_bytes!("../tests/fonts/Iosevka/iosevka-regular.ttf");
        let table = OffsetTable::unpack(&mut Cursor::new(data.to_vec())).unwrap();
        assert_eq!(table.sfnt_version, SfntVersion::TrueType);
        assert_eq!(table.num_tables, 17);
        assert_eq!(table.search_range, 256);
        assert_eq!(table.entry_selector, 4);
        assert_eq!(table.range_shift, 16);

        // should include at least the minimal necessary tables for a font to function correctly
        assert!(
            table.tables.iter().any(|t| t.tag == "cmap"),
            "cmap table missing"
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
            table.tables.iter().any(|t| t.tag == "maxp"),
            "maxp table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "name"),
            "name table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "OS/2"),
            "OS/2 table missing"
        );
        assert!(
            table.tables.iter().any(|t| t.tag == "post"),
            "post table missing"
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        table.pack(&mut buffer).unwrap();
        assert_eq!(
            OffsetTable::unpack(&mut Cursor::new(buffer)).unwrap(),
            table
        );
    }
}