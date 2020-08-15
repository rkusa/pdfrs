mod tables;
mod utils;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::{self, Cursor};
use std::sync::Arc;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures_util::io::{AsyncWrite, AsyncWriteExt};
use tables::offset::{OffsetTable, SfntVersion, TableRecord};
pub use tables::Glyph;
use tables::{FontData, FontTable};

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
    subtable: Arc<tables::cmap::Subtable>,
}

impl OpenTypeFont {
    pub fn from_slice(data: impl AsRef<[u8]>) -> Result<Self, io::Error> {
        let mut cursor = Cursor::new(data.as_ref());
        let offset_table = OffsetTable::unpack(&mut cursor, ())?;

        let head_table = offset_table.unpack_required_table((), &mut cursor)?;
        let hhea_table = offset_table.unpack_required_table((), &mut cursor)?;
        let maxp_table = offset_table.unpack_required_table((), &mut cursor)?;
        let loca_table =
            offset_table.unpack_required_table((&head_table, &maxp_table), &mut cursor)?;

        Ok(OpenTypeFont {
            sfnt_version: offset_table.sfnt_version,
            os2_table: offset_table.unpack_required_table((), &mut cursor)?,
            cmap_table: offset_table.unpack_required_table((), &mut cursor)?,
            glyf_table: offset_table.unpack_required_table(&loca_table, &mut cursor)?,
            hmtx_table: offset_table
                .unpack_required_table((&hhea_table, &maxp_table), &mut cursor)?,
            loca_table,
            head_table,
            hhea_table,
            maxp_table,
            name_table: offset_table.unpack_required_table((), &mut cursor)?,
            post_table: offset_table.unpack_required_table((), &mut cursor)?,
        })
    }

    pub fn font_family_name(&self) -> Option<String> {
        self.name_table.font_family_name()
    }

    pub fn post_script_name(&self) -> Option<String> {
        self.name_table.post_script_name()
    }

    pub fn is_fixed_pitch(&self) -> bool {
        self.post_table.is_fixed_path > 0
    }

    // TODO: re-check
    pub fn is_serif(&self) -> bool {
        matches!(self.os2_table.s_family_class, 1..=7)
    }

    // TODO: re-check
    pub fn is_script(&self) -> bool {
        self.os2_table.s_family_class == 10
    }

    // TODO: re-check
    pub fn is_italic(&self) -> bool {
        self.post_table.italic_angle != 0
    }

    pub fn italic_angle(&self) -> i32 {
        self.post_table.italic_angle
    }

    pub fn units_per_em(&self) -> u16 {
        self.head_table.units_per_em
    }

    pub fn bbox(&self) -> [i16; 4] {
        [
            self.head_table.x_min,
            self.head_table.y_min,
            self.head_table.x_max,
            self.head_table.y_max,
        ]
    }

    pub fn ascent(&self) -> i16 {
        self.os2_table.s_typo_ascender
    }

    pub fn descent(&self) -> i16 {
        self.os2_table.s_typo_descender
    }

    pub fn line_gap(&self) -> i16 {
        self.hhea_table.line_gap
    }

    pub fn cap_height(&self) -> i16 {
        self.os2_table.s_cap_height
    }

    pub fn x_height(&self) -> i16 {
        self.os2_table.sx_height
    }

    pub fn char_width(&self, ch: char) -> u16 {
        let ix = self.glyph_id(u32::from(ch)).unwrap_or(0);
        self.hmtx_table
            .h_metrics
            .get(ix as usize)
            .map(|m| m.advance_width)
            .unwrap_or(0)
    }

    // TODO: return u32?
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        self.cmap_table
            .encoding_records
            .first()
            .and_then(|record| record.subtable.glyph_id(codepoint))
    }

    pub fn subset(&self, chars: impl Iterator<Item = char>) -> Self {
        let subtable = match self.cmap_table.encoding_records.first() {
            Some(r) => r.subtable.clone(),
            // TODO: error instead?
            None => return self.clone(),
        };
        let glyphs = chars
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

        self.subset_from_glyphs(&glyphs)
    }

    pub fn subset_from_glyphs(&self, glyphs: &[Glyph]) -> Self {
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
    pub fn to_vec(&self) -> Result<(Vec<u8>, Vec<u8>), io::Error> {
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
        writer.finish(self.sfnt_version, check_sum_adjustment_offset)
    }

    pub fn to_writer(&self, mut wr: impl io::Write) -> Result<(), io::Error> {
        let (a, b) = self.to_vec()?;
        wr.write_all(&a)?;
        wr.write_all(&b)?;
        Ok(())
    }

    pub async fn to_async_writer(&self, mut wr: impl AsyncWrite + Unpin) -> Result<(), io::Error> {
        let (a, b) = self.to_vec()?;
        wr.write_all(&a).await?;
        wr.write_all(&b).await?;

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

    fn pack<'a, T, U, S>(&mut self, table: &T) -> Result<(), io::Error>
    where
        T: FontTable<'a, U, S>,
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
        if self.buffer.len() % 4 != 0 {
            // align to 4 bytes
            let new_len = self.buffer.len() + (4 - (self.buffer.len() % 4));
            self.buffer.resize(new_len, 0);
        }
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
    ) -> Result<(Vec<u8>, Vec<u8>), io::Error> {
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

        Ok((offset_data, self.buffer))
    }
}

fn check_sum(mut data: impl io::Read) -> u32 {
    let mut sum = 0u32;
    while let Ok(n) = data.read_u32::<BigEndian>() {
        sum = sum.saturating_add(n);
    }
    sum
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

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
        let subset = font.subset("abA".chars());

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
