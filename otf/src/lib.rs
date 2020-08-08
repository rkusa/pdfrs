mod tables;
mod utils;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::{self, Cursor};
use std::rc::Rc;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tables::{FontTable, Glyph};
use utils::limit_read::LimitRead;

#[derive(Debug, PartialEq, Clone)]
pub struct OpenTypeFont {
    sfnt_version: SfntVersion,
    os2_table: tables::os2::Os2Table,
    cmap_subtables: Vec<CmapSubtable>,
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

        let cmap_record = offset_table
            .get_table_record("cmap")
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "cmap table missing"))?;
        let cmap_table: tables::cmap::CmapTable =
            offset_table.unpack_required_table("cmap", (), &mut cursor)?;

        let mut subtables: Vec<(u32, CmapSubtable)> =
            Vec::with_capacity(cmap_table.encoding_records.len());
        for record in &cmap_table.encoding_records {
            let existing_subtable = subtables
                .iter()
                .find(|(offset, _)| record.offset == *offset)
                .map(|(_, subtable)| Rc::clone(&subtable.subtable));
            if let Some(subtable) = existing_subtable {
                subtables.push((
                    record.offset,
                    CmapSubtable {
                        platform_id: record.platform_id,
                        encoding_id: record.encoding_id,
                        subtable,
                    },
                ));
                continue;
            }

            cursor.set_position((cmap_record.offset + record.offset) as u64);
            let subtable = tables::cmap::Subtable::unpack(&mut cursor, ())?;
            subtables.push((
                record.offset,
                CmapSubtable {
                    platform_id: record.platform_id,
                    encoding_id: record.encoding_id,
                    subtable: Rc::new(subtable),
                },
            ));
        }
        let cmap_subtables = subtables.into_iter().map(|(_, st)| st).collect();

        let head_table = offset_table.unpack_required_table("head", (), &mut cursor)?;
        let hhea_table = offset_table.unpack_required_table("hhea", (), &mut cursor)?;
        let maxp_table = offset_table.unpack_required_table("maxp", (), &mut cursor)?;
        let loca_table =
            offset_table.unpack_required_table("loca", (&head_table, &maxp_table), &mut cursor)?;
        Ok(OpenTypeFont {
            sfnt_version: offset_table.sfnt_version,
            os2_table: offset_table.unpack_required_table("OS/2", (), &mut cursor)?,
            cmap_subtables,
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
        self.cmap_subtables
            .first()
            .and_then(|subtable| subtable.subtable.glyph_id(codepoint))
    }

    pub fn subset(&self, text: &str) -> Self {
        let subtable = match self.cmap_subtables.first() {
            Some(s) => s.subtable.clone(),
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
        let mut subsetted_subtables: Vec<(Rc<tables::cmap::Subtable>, Rc<tables::cmap::Subtable>)> =
            Vec::new();
        let cmap_subtables = self
            .cmap_subtables
            .iter()
            .map(|entry| {
                let new_subtable = subsetted_subtables
                    .iter()
                    .find(|(prev, _)| Rc::ptr_eq(prev, &entry.subtable))
                    .map(|(_, new_subtable)| new_subtable.clone())
                    .unwrap_or_else(|| {
                        let new_subtable = Rc::new(entry.subtable.subset(&glyphs, ()).into_owned());
                        subsetted_subtables.push((entry.subtable.clone(), new_subtable.clone()));
                        new_subtable
                    });

                CmapSubtable {
                    platform_id: entry.platform_id,
                    encoding_id: entry.encoding_id,
                    subtable: new_subtable,
                }
            })
            .collect();
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
            cmap_subtables,
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
    pub fn to_writer(&self, mut wr: impl io::Write) -> Result<(), io::Error> {
        let mut tables = Vec::new();
        // reserve space for offset table
        let mut offset: usize = 12 + 10 * 16;

        // OS/2 table
        let mut os2_data = Vec::new();
        self.os2_table.pack(&mut os2_data)?;
        os2_data.resize(os2_data.len() + os2_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "OS/2".to_string(),
            check_sum: check_sum(Cursor::new(&os2_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(os2_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += os2_data.len();

        // cmap subtables
        let mut cmap_subtables_data = Vec::new();
        let mut cmap_table = tables::cmap::CmapTable {
            version: 0,
            num_tables: u16::try_from(self.cmap_subtables.len())
                .ok()
                .unwrap_or(u16::MAX),
            encoding_records: Vec::with_capacity(self.cmap_subtables.len()),
        };
        // reserve cmap table data
        let mut subtable_offset = 4 + self.cmap_subtables.len() * 8;
        let mut written_subtables = Vec::new();
        for subtable in &self.cmap_subtables {
            let prev_offset = written_subtables
                .iter()
                .find(|(_, other)| Rc::ptr_eq(other, &subtable.subtable))
                .map(|(offset, _)| *offset);
            if let Some(prev_offset) = prev_offset {
                cmap_table
                    .encoding_records
                    .push(tables::cmap::EncodingRecord {
                        platform_id: subtable.platform_id,
                        encoding_id: subtable.encoding_id,
                        offset: prev_offset,
                    });
                continue;
            }

            let len_before = cmap_subtables_data.len();
            subtable.subtable.pack(&mut cmap_subtables_data)?; // align to 4 bytes
            let record_offset = u32::try_from(subtable_offset).ok().unwrap_or(u32::MAX);
            cmap_table
                .encoding_records
                .push(tables::cmap::EncodingRecord {
                    platform_id: subtable.platform_id,
                    encoding_id: subtable.encoding_id,
                    offset: record_offset,
                });
            written_subtables.push((record_offset, subtable.subtable.clone()));
            subtable_offset += cmap_subtables_data.len() - len_before;
        }
        cmap_subtables_data.resize(cmap_subtables_data.len() + cmap_subtables_data.len() % 4, 0);

        // cmap table
        let mut cmap_data = Vec::new();
        cmap_table.pack(&mut cmap_data)?;
        cmap_data.resize(cmap_data.len() + cmap_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "cmap".to_string(),
            check_sum: check_sum(Cursor::new(&cmap_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(cmap_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += cmap_data.len() + cmap_subtables_data.len();

        // glyf table
        let mut glyf_data = Vec::new();
        self.glyf_table.pack(&mut glyf_data)?;
        glyf_data.resize(glyf_data.len() + glyf_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "glyf".to_string(),
            check_sum: check_sum(Cursor::new(&glyf_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(glyf_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += glyf_data.len();

        // head table
        let mut head_data = Vec::new();
        self.head_table.pack(&mut head_data)?;
        head_data.resize(head_data.len() + head_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "head".to_string(),
            check_sum: check_sum(Cursor::new(&head_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(head_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += head_data.len();

        // hhea table
        let mut hhea_data = Vec::new();
        self.hhea_table.pack(&mut hhea_data)?;
        hhea_data.resize(hhea_data.len() + hhea_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "hhea".to_string(),
            check_sum: check_sum(Cursor::new(&hhea_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(hhea_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += hhea_data.len();

        // hmtx table
        let mut hmtx_data = Vec::new();
        self.hmtx_table.pack(&mut hmtx_data)?;
        hmtx_data.resize(hmtx_data.len() + hmtx_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "hmtx".to_string(),
            check_sum: check_sum(Cursor::new(&hmtx_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(hmtx_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += hmtx_data.len();

        // loca table
        let mut loca_data = Vec::new();
        self.loca_table.pack(&mut loca_data)?;
        loca_data.resize(loca_data.len() + loca_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "loca".to_string(),
            check_sum: check_sum(Cursor::new(&loca_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(loca_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += loca_data.len();

        // maxp table
        let mut maxp_data = Vec::new();
        self.maxp_table.pack(&mut maxp_data)?;
        maxp_data.resize(maxp_data.len() + maxp_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "maxp".to_string(),
            check_sum: check_sum(Cursor::new(&maxp_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(maxp_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += maxp_data.len();

        // name table
        let mut name_data = Vec::new();
        self.name_table.pack(&mut name_data)?;
        name_data.resize(name_data.len() + name_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "name".to_string(),
            check_sum: check_sum(Cursor::new(&name_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(name_data.len()).ok().unwrap_or(u32::MAX),
        });
        offset += name_data.len();

        // post table
        let mut post_data = Vec::new();
        self.post_table.pack(&mut post_data)?;
        post_data.resize(post_data.len() + post_data.len() % 4, 0); // align to 4 bytes
        tables.push(TableRecord {
            tag: "post".to_string(),
            check_sum: check_sum(Cursor::new(&post_data)),
            offset: u32::try_from(offset).ok().unwrap_or(u32::MAX),
            length: u32::try_from(post_data.len()).ok().unwrap_or(u32::MAX),
        });
        // offset += post_data.len();

        // TODO: update head.checkSumAdjustment, see
        // https://docs.microsoft.com/en-us/typography/opentype/spec/otff#calculating-checksums

        let num_tables = u16::try_from(tables.len()).ok().unwrap_or(u16::MAX);
        let x = 2u16.pow((num_tables as f32).log2() as u32);
        let search_range = x * 16;
        let entry_selector = (x as f32).log2() as u16;
        let range_shift = num_tables * 16 - search_range;

        let offset_table = OffsetTable {
            sfnt_version: self.sfnt_version,
            num_tables,
            search_range,
            entry_selector,
            range_shift,
            tables,
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
        let mut cursor = Cursor::new(&mut head_data);
        cursor.set_position(8);
        cursor.write_u32::<BigEndian>(check_sum_adjustment)?;

        wr.write_all(&offset_data)?;
        wr.write_all(&os2_data)?;
        wr.write_all(&cmap_data)?;
        wr.write_all(&cmap_subtables_data)?;
        wr.write_all(&glyf_data)?;
        wr.write_all(&head_data)?;
        wr.write_all(&hhea_data)?;
        wr.write_all(&hmtx_data)?;
        wr.write_all(&loca_data)?;
        wr.write_all(&maxp_data)?;
        wr.write_all(&name_data)?;
        wr.write_all(&post_data)?;

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
        let mut limit_read = LimitRead::new(cursor, record.length as usize);
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

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
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

    fn unpack<R: io::Read>(rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
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

    fn unpack<R: io::Read>(rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
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
        let data = include_bytes!("../tests/fonts/Iosevka/iosevka-regular.ttf");
        let table = OffsetTable::unpack(&mut Cursor::new(data.to_vec()), ()).unwrap();
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
            OffsetTable::unpack(&mut Cursor::new(buffer), ()).unwrap(),
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
            cmap_subtables,
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
        assert_eq!(cmap_subtables, subset.cmap_subtables);
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
