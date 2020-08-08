mod tables;
mod utils;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::{self, Cursor, Read};
use std::rc::Rc;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tables::{FontTable, Glyph, NamedTable};
use utils::limit_read::LimitRead;

#[derive(Debug, PartialEq, Clone)]
pub struct OpenTypeFont {
    sfnt_version: SfntVersion,
    os2_table: tables::os2::Os2Table,
    cmap_table: tables::cmap::CmapTable,
    glyf_table: tables::glyf::GlyfTable,
    head_table: tables::head::HeadTable,
    hhea_table: tables::hhea::HheaTable,
    hmtx_table: tables::hmtx::HmtxTable,
    loca_table: tables::loca::LocaTable,
    maxp_table: tables::maxp::MaxpTable,
    name_table: tables::name::NameTable,
    post_table: tables::post::PostTable,
}

#[derive(Debug, PartialEq, Clone)]
struct CmapSubtable {
    platform_id: u16,
    encoding_id: u16,
    subtable: Rc<tables::cmap::Subtable>,
}

impl OpenTypeFont {
    pub fn from_slice(data: impl AsRef<[u8]>) -> Result<Self, io::Error> {
        let mut cursor = Cursor::new(data.as_ref());
        let offset_table = OffsetTable::unpack(&mut cursor, ())?;

        let head_table = offset_table.unpack_required_table("head", (), &mut cursor)?;
        let hhea_table = offset_table.unpack_required_table("hhea", (), &mut cursor)?;
        let maxp_table = offset_table.unpack_required_table("maxp", (), &mut cursor)?;
        let loca_table =
            offset_table.unpack_required_table("loca", (&head_table, &maxp_table), &mut cursor)?;

        Ok(OpenTypeFont {
            sfnt_version: offset_table.sfnt_version,
            os2_table: offset_table.unpack_required_table("OS/2", (), &mut cursor)?,
            cmap_table: offset_table.unpack_required_table("cmap", (), &mut cursor)?,
            glyf_table: offset_table.unpack_required_table("glyf", &loca_table, &mut cursor)?,
            hmtx_table: offset_table.unpack_required_table(
                "hmtx",
                (&hhea_table, &maxp_table),
                &mut cursor,
            )?,
            loca_table,
            head_table,
            hhea_table,
            maxp_table,
            name_table: offset_table.unpack_required_table("name", (), &mut cursor)?,
            post_table: offset_table.unpack_required_table("post", (), &mut cursor)?,
        })
    }

    // TODO: return u32?
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        self.cmap_table
            .encoding_records
            .first()
            .and_then(|record| record.subtable.glyph_id(codepoint))
    }

    pub fn subset(&self, text: &str) -> Self {
        let subtable = match self.cmap_table.encoding_records.first() {
            Some(r) => r.subtable.clone(),
            // TODO: error instead?
            None => return self.clone(),
        };
        let glyphs = text
            .chars()
            .filter_map(|c| {
                subtable
                    .glyph_id(u32::from(c))
                    .map(|index| (index, u32::from(c)))
            })
            .fold(HashMap::new(), |mut glyphs, (i, c)| {
                let glyph = glyphs.entry(i).or_insert_with(|| Glyph {
                    index: i,
                    code_points: Vec::with_capacity(1),
                });
                glyph.code_points.push(c);
                glyphs
            })
            .into_iter()
            .map(|(_, g)| g)
            .collect::<Vec<_>>();

        let glyphs = self.glyf_table.expand_composite_glyphs(&glyphs);

        let os2_table = self.os2_table.subset(&glyphs, ()).into_owned();
        let cmap_table = self.cmap_table.subset(&glyphs, ()).into_owned();
        let glyf_table = self.glyf_table.subset(&glyphs, ()).into_owned();
        let loca_table = self.loca_table.subset(&glyphs, &glyf_table).into_owned();
        let head_table = self
            .head_table
            .subset(&glyphs, (&glyf_table, &loca_table))
            .into_owned();
        let hmtx_table = self.hmtx_table.subset(&glyphs, ()).into_owned();
        let hhea_table = self
            .hhea_table
            .subset(&glyphs, (&head_table, &hmtx_table))
            .into_owned();
        let maxp_table = self.maxp_table.subset(&glyphs, ()).into_owned();
        let name_table = self.name_table.subset(&glyphs, ()).into_owned();
        let post_table = self.post_table.subset(&glyphs, ()).into_owned();

        OpenTypeFont {
            sfnt_version: self.sfnt_version,
            os2_table,
            cmap_table,
            glyf_table,
            head_table,
            hhea_table,
            hmtx_table,
            loca_table,
            maxp_table,
            name_table,
            post_table,
        }
    }

    /// Note: currently skips all other tables of the font that are not known to the library.
    pub fn to_writer(&self, wr: impl io::Write) -> Result<(), io::Error> {
        let mut writer = FontWriter::new(10);
        writer.pack(&self.os2_table)?;
        writer.pack(&self.cmap_table)?;
        writer.pack(&self.glyf_table)?;
        let check_sum_adjustment_offset = writer.offset() + 8;
        writer.pack(&self.head_table)?;
        writer.pack(&self.hhea_table)?;
        writer.pack(&self.hmtx_table)?;
        writer.pack(&self.loca_table)?;
        writer.pack(&self.maxp_table)?;
        writer.pack(&self.name_table)?;
        writer.pack(&self.post_table)?;
        writer.finish(self.sfnt_version, check_sum_adjustment_offset, wr)?;

        Ok(())
    }
}

struct FontWriter {
    tables: Vec<TableRecord>,
    buffer: Vec<u8>,
}

impl FontWriter {
    fn new(len: usize) -> Self {
        FontWriter {
            tables: Vec::with_capacity(len),
            buffer: Vec::new(), // TODO: guess a starting size?
        }
    }

    fn offset_table_len(&self) -> usize {
        12 + self.tables.capacity() * 16
    }

    fn offset(&self) -> usize {
        self.offset_table_len() + self.buffer.len()
    }

    fn pack<'a, T, UD, SD>(&mut self, table: &T) -> Result<(), io::Error>
    where
        T: FontTable<'a, UnpackDep = UD, SubsetDep = SD> + NamedTable,
    {
        if self.tables.len() == self.tables.capacity() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Cannot write another table, already wrote {} tables",
                    self.tables.len()
                ),
            ));
        }

        let start = self.buffer.len();
        table.pack(&mut self.buffer)?;
        let len = self.buffer.len() - start;
        self.buffer
            .resize(self.buffer.len() + self.buffer.len() % 4, 0); // align to 4 bytes
        self.tables.push(TableRecord {
            tag: T::name().to_string(),
            check_sum: check_sum(&self.buffer[start..]),
            offset: u32::try_from(self.offset_table_len() + start)
                .ok()
                .unwrap_or(u32::MAX),
            length: u32::try_from(len).ok().unwrap_or(u32::MAX),
        });

        Ok(())
    }

    fn finish(
        mut self,
        sfnt_version: SfntVersion,
        check_sum_adjustment_offset: usize,
        mut wr: impl io::Write,
    ) -> Result<(), io::Error> {
        let num_tables = u16::try_from(self.tables.len()).ok().unwrap_or(u16::MAX);
        let x = 2u16.pow((num_tables as f32).log2() as u32);
        let search_range = x * 16;
        let entry_selector = (x as f32).log2() as u16;
        let range_shift = num_tables * 16 - search_range;

        let check_sum_adjustment_offset = check_sum_adjustment_offset - self.offset_table_len();
        let offset_table = OffsetTable {
            sfnt_version,
            num_tables,
            search_range,
            entry_selector,
            range_shift,
            tables: self.tables,
        };
        let mut offset_data = Vec::new();
        offset_table.pack(&mut offset_data)?;

        // calculate and write head.check_sum_adjustment
        let mut check_sum_adjustment = check_sum(Cursor::new(&offset_data));
        for record in &offset_table.tables {
            check_sum_adjustment = check_sum_adjustment.saturating_add(record.check_sum);
        }
        let check_sum_adjustment = 0xB1B0AFBAu32.saturating_sub(check_sum_adjustment);

        // inject check_sum_adjustment into head table data
        (&mut self.buffer[check_sum_adjustment_offset..])
            .write_u32::<BigEndian>(check_sum_adjustment)?;

        wr.write_all(&offset_data)?;
        wr.write_all(&self.buffer)?;

        Ok(())
    }
}

fn check_sum(mut data: impl io::Read) -> u32 {
    let mut sum = 0u32;
    while let Ok(n) = data.read_u32::<BigEndian>() {
        sum = sum.saturating_add(n);
    }
    sum
}

/// This table contains a dictionary of all font tables included in the file.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/otff
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6.html
#[derive(Debug, PartialEq)]
struct OffsetTable {
    /// OpenType fonts that contain TrueType outlines should use the value of 0x00010000. OpenType
    /// fonts containing CFF data (version 1 or 2) should use 0x4F54544F ('OTTO', when
    /// re-interpreted as a Tag).
    sfnt_version: SfntVersion,
    /// Number of tables.
    num_tables: u16,
    /// (Maximum power of 2 <= numTables) x 16.
    search_range: u16,
    /// Log2(maximum power of 2 <= numTables).
    entry_selector: u16,
    /// NumTables x 16-searchRange.
    range_shift: u16,
    /// Table records of the front. Expected to be ordered ascending by their tag.
    tables: Vec<TableRecord>,
}

impl OffsetTable {
    fn get_table_record(&self, tag: &str) -> Option<&TableRecord> {
        self.tables
            .binary_search_by(|r| r.tag.as_str().cmp(tag))
            .ok()
            .and_then(|i| self.tables.get(i))
    }

    fn unpack_table<'a, T, R, UD, SD>(
        &self,
        tag: &str,
        dep: UD,
        cursor: &mut Cursor<R>,
    ) -> Result<Option<T>, io::Error>
    where
        R: io::Read + AsRef<[u8]>,
        T: FontTable<'a, UnpackDep = UD, SubsetDep = SD>,
    {
        // TODO: return Option for non-required tables?
        let record = match self.get_table_record(tag) {
            Some(record) => record,
            None => return Ok(None),
        };

        cursor.set_position(record.offset as u64);
        let mut limit_read = Cursor::new(LimitRead::from_cursor(cursor, record.length as usize));
        Ok(Some(T::unpack(&mut limit_read, dep)?))
    }

    fn unpack_required_table<'a, T, R, UD, SD>(
        &self,
        tag: &str,
        dep: UD,
        cursor: &mut Cursor<R>,
    ) -> Result<T, io::Error>
    where
        R: io::Read + AsRef<[u8]>,
        T: FontTable<'a, UnpackDep = UD, SubsetDep = SD>,
    {
        self.unpack_table::<T, R, UD, SD>(tag, dep, cursor)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, format!("{} table missing", tag)))
    }
}

impl<'a> FontTable<'a> for OffsetTable {
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
enum SfntVersion {
    TrueType,
    CFF,
}

impl<'a> FontTable<'a> for SfntVersion {
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
struct TableRecord {
    tag: String,
    check_sum: u32,
    offset: u32,
    length: u32,
}

impl<'a> FontTable<'a> for TableRecord {
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
    use std::io::Cursor;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_offset_table_encode_decode() {
        let data = include_bytes!("../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
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

    #[test]
    fn test_writing_font() {
        let data = include_bytes!("../tests/fonts/Iosevka/iosevka-regular.ttf");
        let font = OpenTypeFont::from_slice(&data[..]).unwrap();

        let mut data = Vec::new();
        font.to_writer(&mut data).unwrap();
        let mut rewritten_font = OpenTypeFont::from_slice(data).unwrap();

        assert_ne!(
            rewritten_font.head_table.check_sum_adjustment,
            font.head_table.check_sum_adjustment
        );
        rewritten_font.head_table.check_sum_adjustment = font.head_table.check_sum_adjustment;
        assert_eq!(
            rewritten_font, font,
            "Re-written font does not match original font"
        );
    }

    #[test]
    fn test_reparse_subset() {
        let data = include_bytes!("../tests/fonts/Iosevka/iosevka-regular.ttf");
        let font = OpenTypeFont::from_slice(&data[..]).unwrap();
        let subset = font.subset("abA");

        let mut data = Vec::new();
        subset.to_writer(&mut data).unwrap();
        let mut rewritten_subset = OpenTypeFont::from_slice(data).unwrap();

        assert_ne!(
            rewritten_subset.head_table.check_sum_adjustment,
            font.head_table.check_sum_adjustment
        );
        rewritten_subset.head_table.check_sum_adjustment = font.head_table.check_sum_adjustment;
        let OpenTypeFont {
            sfnt_version,
            os2_table,
            cmap_table,
            glyf_table,
            head_table,
            hhea_table,
            hmtx_table,
            loca_table,
            maxp_table,
            name_table,
            post_table,
        } = rewritten_subset;
        assert_eq!(sfnt_version, subset.sfnt_version);
        assert_eq!(os2_table, subset.os2_table);
        assert_eq!(cmap_table, subset.cmap_table);
        assert_eq!(glyf_table, subset.glyf_table);
        assert_eq!(head_table, subset.head_table);
        assert_eq!(hhea_table, subset.hhea_table);
        assert_eq!(hmtx_table, subset.hmtx_table);
        assert_eq!(loca_table, subset.loca_table);
        assert_eq!(maxp_table, subset.maxp_table);
        assert_eq!(name_table, subset.name_table);
        assert_eq!(post_table, subset.post_table);
    }
}
