use std::borrow::Cow;
use std::io::{self, Cursor};

use super::glyf::GlyfTable;
use super::loca::{Format as LocaFormat, LocaTable};
use super::{FontData, FontTable, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// See https://docs.microsoft.com/en-us/typography/opentype/spec/head
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(Default))]
pub struct HeadTable {
    /// Major version number of the font header table — set to 1.
    pub(crate) major_version: u16,
    /// Minor version number of the font header table — set to 0.
    pub(crate) minor_version: u16,
    /// Set by font manufacturer.
    pub(crate) font_revision: (i16, u16),
    /// To compute: set it to 0, sum the entire font as uint32, then store 0xB1B0AFBA - sum. If the
    /// font is used as a component in a font collection file, the value of this field will be
    /// invalidated by changes to the file structure and font table directory, and must be ignored.
    pub(crate) check_sum_adjustment: u32,
    /// Set to 0x5F0F3CF5.
    pub(crate) magic_number: u32,
    /// Bit 0: Baseline for font at y=0;
    /// Bit 1: Left sidebearing point at x=0 (relevant only for TrueType rasterizers) — see the note
    ///        below regarding variable fonts;
    /// Bit 2: Instructions may depend on point size;
    /// Bit 3: Force ppem to integer values for all internal scaler math; may use fractional ppem
    ///        sizes if this bit is clear;
    /// Bit 4: Instructions may alter advance width (the advance widths might not scale linearly);
    /// Bit 5: This bit is not used in OpenType, and should not be set in order to ensure compatible
    ///         behavior on all platforms.
    /// Bits 6–10: These bits are not used in Opentype and should always be cleared.
    /// Bit 11: Font data is “lossless” as a result of having been subjected to optimizing
    ///         transformation and/or compression.
    /// Bit 12: Font converted (produce compatible metrics)
    /// Bit 13: Font optimized for ClearType™.
    /// Bit 14: Last Resort font. If set, indicates that the glyphs encoded in the 'cmap' subtables
    ///         are simply generic symbolic representations of code point ranges and don’t truly
    ///         represent support for those code points. If unset, indicates that the glyphs encoded
    ///         in the 'cmap' subtables represent proper support for those code points.
    /// Bit 15: Reserved, set to 0
    pub(crate) flags: u16,
    /// Set to a value from 16 to 16384. Any value in this range is valid.
    pub(crate) units_per_em: u16,
    /// Number of seconds since 12:00 midnight that started January 1st 1904 in GMT/UTC time zone.
    pub(crate) created: i64,
    /// Number of seconds since 12:00 midnight that started January 1st 1904 in GMT/UTC time zone.
    pub(crate) modified: i64,
    /// Min x of all glyph bounding boxes.
    pub(crate) x_min: i16,
    /// Min y of all glyph bounding boxes.
    pub(crate) y_min: i16,
    /// Max x of all glyph bounding boxes.
    pub(crate) x_max: i16,
    /// Max y of all glyph bounding boxes.
    pub(crate) y_max: i16,
    /// Bit 0: Bold (if set to 1);
    /// Bit 1: Italic (if set to 1)
    /// Bit 2: Underline (if set to 1)
    /// Bit 3: Outline (if set to 1)
    /// Bit 4: Shadow (if set to 1)
    /// Bit 5: Condensed (if set to 1)
    /// Bit 6: Extended (if set to 1)
    /// Bits 7–15: Reserved (set to 0).
    pub(crate) mac_style: u16,
    /// Smallest readable size in pixels.
    pub(crate) lowest_rec_ppem: u16,
    /// Deprecated (Set to 2).
    pub(crate) font_direction_hint: i16,
    /// 0 for short offsets (Offset16), 1 for long (Offset32).
    pub(crate) index_to_loc_format: i16,
    /// 0 for current format.
    pub(crate) glyph_data_format: i16,
}

impl<'a> FontTable<'a, (), (), (&'a GlyfTable, &'a LocaTable)> for HeadTable {
    fn name() -> &'static str {
        "head"
    }
}

impl<'a> FontData<'a> for HeadTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = (&'a GlyfTable, &'a LocaTable);

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let major_version = rd.read_u16::<BigEndian>()?;
        let minor_version = rd.read_u16::<BigEndian>()?;

        let decimal = rd.read_i16::<BigEndian>()?;
        let fraction = rd.read_u16::<BigEndian>()?;

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

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
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

    fn subset(&'a self, _glyphs: &[Glyph], (glyf, loca): Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Owned(HeadTable {
            x_min: glyf
                .glyphs
                .iter()
                .filter_map(|g| g.as_ref().map(|d| d.x_min))
                .min()
                .unwrap_or(0),
            y_min: glyf
                .glyphs
                .iter()
                .filter_map(|g| g.as_ref().map(|d| d.y_min))
                .min()
                .unwrap_or(0),
            x_max: glyf
                .glyphs
                .iter()
                .filter_map(|g| g.as_ref().map(|d| d.x_max))
                .max()
                .unwrap_or(0),
            y_max: glyf
                .glyphs
                .iter()
                .filter_map(|g| g.as_ref().map(|d| d.y_max))
                .max()
                .unwrap_or(0),
            index_to_loc_format: match loca.format {
                LocaFormat::Short => 0,
                LocaFormat::Long => 1,
            },
            ..self.clone()
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tables::glyf::{GlyphData, GlyphDescription};
    use crate::OffsetTable;

    #[test]
    fn test_head_table_encode_decode() {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let head_table: HeadTable = table.unpack_required_table((), &mut cursor).unwrap();

        assert_eq!(head_table.major_version, 1);
        assert_eq!(head_table.minor_version, 0);
        // font_revision = 3.031
        assert_eq!(head_table.font_revision.0, 3);
        assert!((head_table.font_revision.1 as f32 / 65535.0 - 0.031).abs() < 0.00001);
        assert_eq!(head_table.check_sum_adjustment, 3547005195);
        assert_eq!(head_table.magic_number, 1594834165);
        assert_eq!(head_table.flags, 13);
        assert_eq!(head_table.units_per_em, 1000);
        assert_eq!(head_table.created, 3562553439);
        assert_eq!(head_table.modified, 3678044538);
        assert_eq!(head_table.x_min, -1000);
        assert_eq!(head_table.y_min, -505);
        assert_eq!(head_table.x_max, 1134);
        assert_eq!(head_table.y_max, 1188);
        assert_eq!(head_table.mac_style, 0);
        assert_eq!(head_table.lowest_rec_ppem, 8);
        assert_eq!(head_table.font_direction_hint, 0);
        assert_eq!(head_table.index_to_loc_format, 1);
        assert_eq!(head_table.glyph_data_format, 0);

        // re-pack and compare
        let mut buffer = Vec::new();
        head_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            HeadTable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            head_table
        );
    }

    #[test]
    fn test_head_table_subset() {
        let g2 = GlyphData {
            number_of_contours: 1,
            x_min: 1,
            y_min: 2,
            x_max: 3,
            y_max: 4,
            description: GlyphDescription::Simple(Vec::new()),
        };
        let g4 = GlyphData {
            number_of_contours: 2,
            x_min: 4,
            y_min: 3,
            x_max: 2,
            y_max: 1,
            description: GlyphDescription::Simple(Vec::new()),
        };
        let glyf = GlyfTable {
            glyphs: vec![None, Some(g2), None, Some(g4), None],
        };
        let loca = LocaTable {
            offsets: Vec::new(),
            format: LocaFormat::Long,
        };

        let head = HeadTable::default();
        let subset = head.subset(&[], (&glyf, &loca));
        assert_eq!(
            subset.as_ref(),
            &HeadTable {
                x_min: 1,
                y_min: 2,
                x_max: 3,
                y_max: 4,
                index_to_loc_format: 1,
                ..Default::default()
            }
        )
    }
}
