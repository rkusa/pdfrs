use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{self, Cursor};

use super::head::HeadTable;
use super::hmtx::HmtxTable;
use super::{FontData, FontTable, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table contains information for horizontal layout.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/hhea
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6hhea.html
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(Default))]
pub struct HheaTable {
    /// Major version number of the horizontal header table — set to 1.
    pub major_version: u16,
    /// Minor version number of the horizontal header table — set to 0.
    pub minor_version: u16,
    /// Distance from baseline of highest ascender.
    pub ascent: i16,
    /// Distance from baseline of lowest descender
    pub descent: i16,
    /// Typographic line gap.
    pub line_gap: i16,
    /// Maximum advance width value in 'hmtx' table.
    pub advance_width_max: u16,
    /// Minimum left sidebearing value in 'hmtx' table.
    pub min_left_side_bearing: i16,
    /// Minimum right sidebearing value; calculated as Min(aw - lsb - (xMax - xMin)).
    pub min_right_side_bearing: i16,
    /// Max(lsb + (xMax - xMin)).
    pub x_max_extent: i16,
    /// Used to calculate the slope of the cursor (rise/run); 1 for vertical.
    pub caret_slope_rise: i16,
    /// 0 for vertical.
    pub caret_slope_run: i16,
    /// The amount by which a slanted highlight on a glyph needs to be shifted to produce the best
    /// appearance.
    pub caret_offset: i16,
    /// 0 for current format.
    pub metric_data_format: i16,
    /// Number of hMetric entries in 'hmtx' table
    pub number_of_h_metrics: u16,
}

impl<'a> FontTable<'a, (), (), (&'a HeadTable, &'a HmtxTable)> for HheaTable {
    fn name() -> &'static str {
        "hhea"
    }
}

impl<'a> FontData<'a> for HheaTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = (&'a HeadTable, &'a HmtxTable);

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let major_version = rd.read_u16::<BigEndian>()?;
        let minor_version = rd.read_u16::<BigEndian>()?;
        let ascent = rd.read_i16::<BigEndian>()?;
        let descent = rd.read_i16::<BigEndian>()?;
        let line_gap = rd.read_i16::<BigEndian>()?;
        let advance_width_max = rd.read_u16::<BigEndian>()?;
        let min_left_side_bearing = rd.read_i16::<BigEndian>()?;
        let min_right_side_bearing = rd.read_i16::<BigEndian>()?;
        let x_max_extent = rd.read_i16::<BigEndian>()?;
        let caret_slope_rise = rd.read_i16::<BigEndian>()?;
        let caret_slope_run = rd.read_i16::<BigEndian>()?;
        let caret_offset = rd.read_i16::<BigEndian>()?;
        // 4 times reserved
        for _ in 0..4 {
            rd.read_i16::<BigEndian>()?;
        }

        Ok(HheaTable {
            major_version,
            minor_version,
            ascent,
            descent,
            line_gap,
            advance_width_max,
            min_left_side_bearing,
            min_right_side_bearing,
            x_max_extent,
            caret_slope_rise,
            caret_slope_run,
            caret_offset,
            metric_data_format: rd.read_i16::<BigEndian>()?,
            number_of_h_metrics: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        // TODO: update values based on hmax table
        wr.write_u16::<BigEndian>(self.major_version)?;
        wr.write_u16::<BigEndian>(self.minor_version)?;
        wr.write_i16::<BigEndian>(self.ascent)?;
        wr.write_i16::<BigEndian>(self.descent)?;
        wr.write_i16::<BigEndian>(self.line_gap)?;
        wr.write_u16::<BigEndian>(self.advance_width_max)?;
        wr.write_i16::<BigEndian>(self.min_left_side_bearing)?;
        wr.write_i16::<BigEndian>(self.min_right_side_bearing)?;
        wr.write_i16::<BigEndian>(self.x_max_extent)?;
        wr.write_i16::<BigEndian>(self.caret_slope_rise)?;
        wr.write_i16::<BigEndian>(self.caret_slope_run)?;
        wr.write_i16::<BigEndian>(self.caret_offset)?;
        wr.write_i16::<BigEndian>(0)?;
        wr.write_i16::<BigEndian>(0)?;
        wr.write_i16::<BigEndian>(0)?;
        wr.write_i16::<BigEndian>(0)?;
        wr.write_i16::<BigEndian>(self.metric_data_format)?;
        wr.write_u16::<BigEndian>(self.number_of_h_metrics)?;
        Ok(())
    }

    fn subset(&'a self, _glyphs: &[Glyph], (head, hmtx): Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        let max_width = head.x_max.saturating_sub(head.x_min);
        Cow::Owned(HheaTable {
            advance_width_max: hmtx
                .h_metrics
                .iter()
                .map(|m| m.advance_width)
                .max()
                .unwrap_or(0),
            min_left_side_bearing: hmtx.left_side_bearings.iter().cloned().min().unwrap_or(0),
            // Min(aw - lsb - (xMax - xMin))
            min_right_side_bearing: hmtx
                .h_metrics
                .iter()
                .map(|m| i16::try_from(m.advance_width).ok().unwrap_or(i16::MAX) - m.lsb)
                .min()
                .unwrap_or(0)
                .saturating_sub(max_width),
            // Max(lsb + (xMax - xMin))
            x_max_extent: hmtx
                .h_metrics
                .iter()
                .map(|m| m.lsb)
                .chain(hmtx.left_side_bearings.iter().cloned())
                .max()
                .unwrap_or(0)
                .saturating_add(max_width),
            number_of_h_metrics: u16::try_from(hmtx.h_metrics.len()).ok().unwrap_or(u16::MAX),
            ..self.to_owned()
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tables::hmtx::LongHorMetric;
    use crate::OffsetTable;

    #[test]
    fn test_hhea_table_encode_decode() {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let hhea_table: HheaTable = table.unpack_required_table((), &mut cursor).unwrap();

        assert_eq!(hhea_table.major_version, 1);
        assert_eq!(hhea_table.minor_version, 0);
        assert_eq!(hhea_table.ascent, 977);
        assert_eq!(hhea_table.descent, -205);
        assert_eq!(hhea_table.line_gap, 67);
        assert_eq!(hhea_table.advance_width_max, 1000);
        assert_eq!(hhea_table.min_left_side_bearing, -1000);
        assert_eq!(hhea_table.min_right_side_bearing, -1000);
        assert_eq!(hhea_table.x_max_extent, 1134);
        assert_eq!(hhea_table.caret_slope_rise, 1);
        assert_eq!(hhea_table.caret_slope_run, 0);
        assert_eq!(hhea_table.caret_offset, 0);
        assert_eq!(hhea_table.metric_data_format, 0);
        assert_eq!(hhea_table.number_of_h_metrics, 8575);

        // re-pack and compare
        let mut buffer = Vec::new();
        hhea_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            HheaTable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            hhea_table
        );
    }

    #[test]
    fn test_hhea_table_subset() {
        let head = HeadTable {
            x_min: 10,
            x_max: 20,
            ..Default::default()
        };
        let hmtx = HmtxTable {
            h_metrics: vec![
                LongHorMetric {
                    advance_width: 50,
                    lsb: 3,
                },
                LongHorMetric {
                    advance_width: 70,
                    lsb: 6,
                },
            ],
            left_side_bearings: vec![-3, 9],
        };
        let hhea = HheaTable::default();
        let subset = hhea.subset(
            &[Glyph::new(0), Glyph::new(1), Glyph::new(2), Glyph::new(3)],
            (&head, &hmtx),
        );
        assert_eq!(
            subset.as_ref(),
            &HheaTable {
                advance_width_max: 70,
                min_left_side_bearing: -3,
                min_right_side_bearing: 37,
                x_max_extent: 19,
                number_of_h_metrics: 2,
                ..hhea
            }
        );
    }
}
