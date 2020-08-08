use std::borrow::Cow;
use std::io;

use super::hhea::HheaTable;
use super::maxp::MaxpTable;
use super::{FontTable, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table contains glyph metrics used for horizontal text layout.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/hmtx
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6hmtx.html
#[derive(Debug, PartialEq, Clone)]
pub struct HmtxTable {
    /// Paired advance width and left side bearing values for each glyph. Records are indexed by
    /// glyph ID.
    pub(crate) h_metrics: Vec<LongHorMetric>,
    /// Left side bearings for glyph IDs greater than or equal to numberOfHMetrics.
    pub(crate) left_side_bearings: Vec<i16>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LongHorMetric {
    /// Advance width, in font design units.
    pub(crate) advance_width: u16,
    /// Glyph left side bearing, in font design units.
    pub(crate) lsb: i16,
}

impl<'a> FontTable<'a> for HmtxTable {
    type UnpackDep = (&'a HheaTable, &'a MaxpTable);
    type SubsetDep = ();

    fn unpack<R: io::Read>(
        mut rd: &mut R,
        (hhea, maxp): Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let mut h_metrics = Vec::with_capacity(hhea.number_of_h_metrics as usize);
        for _ in 0..hhea.number_of_h_metrics {
            h_metrics.push(LongHorMetric::unpack(&mut rd, ())?);
        }

        let mut left_side_bearings =
            vec![0; maxp.num_glyphs().saturating_sub(hhea.number_of_h_metrics) as usize];
        rd.read_i16_into::<BigEndian>(&mut left_side_bearings)?;

        Ok(HmtxTable {
            h_metrics,
            left_side_bearings,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        if self.h_metrics.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot write more than `u16::MAX` h_metrics",
            ));
        }
        for metric in &self.h_metrics {
            metric.pack(&mut wr)?;
        }

        if self.left_side_bearings.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot write more than `u16::MAX` left_side_bearings",
            ));
        }
        for bearing in &self.left_side_bearings {
            wr.write_i16::<BigEndian>(*bearing)?;
        }
        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        let (h_metrics, left_side_bearings) = glyphs
            .iter()
            .partition::<Vec<&Glyph>, _>(|g| (g.index as usize) < self.h_metrics.len());
        let h_metrics = h_metrics
            .into_iter()
            .filter_map(|g| self.h_metrics.get(g.index as usize))
            .cloned()
            .collect();
        let left_side_bearings = left_side_bearings
            .into_iter()
            .filter_map(|g| {
                self.left_side_bearings
                    .get((g.index as usize) - self.h_metrics.len())
            })
            .cloned()
            .collect();
        Cow::Owned(HmtxTable {
            h_metrics,
            left_side_bearings,
        })
    }
}

impl<'a> FontTable<'a> for LongHorMetric {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
        Ok(LongHorMetric {
            advance_width: rd.read_u16::<BigEndian>()?,
            lsb: rd.read_i16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.advance_width)?;
        wr.write_i16::<BigEndian>(self.lsb)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_hmtx_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let hhea_table: HheaTable = table
            .unpack_required_table("hhea", (), &mut cursor)
            .unwrap();
        let maxp_table: MaxpTable = table
            .unpack_required_table("maxp", (), &mut cursor)
            .unwrap();
        let hmtx_table: HmtxTable = table
            .unpack_required_table("hmtx", (&hhea_table, &maxp_table), &mut cursor)
            .unwrap();

        assert_eq!(
            hmtx_table.h_metrics.len(),
            hhea_table.number_of_h_metrics as usize
        );
        assert_eq!(
            hmtx_table.left_side_bearings.len(),
            (maxp_table.num_glyphs() - hhea_table.number_of_h_metrics) as usize
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        hmtx_table.pack(&mut buffer).unwrap();
        assert_eq!(
            HmtxTable::unpack(&mut Cursor::new(buffer), (&hhea_table, &maxp_table)).unwrap(),
            hmtx_table
        );
    }

    #[test]
    fn test_hmtx_table_subset() {
        let metric1 = LongHorMetric {
            advance_width: 1,
            lsb: 1,
        };
        let metric2 = LongHorMetric {
            advance_width: 2,
            lsb: 2,
        };
        let metric3 = LongHorMetric {
            advance_width: 3,
            lsb: 3,
        };
        let metric4 = LongHorMetric {
            advance_width: 4,
            lsb: 4,
        };
        let metric5 = LongHorMetric {
            advance_width: 5,
            lsb: 5,
        };

        let hmtx = HmtxTable {
            h_metrics: vec![
                metric1,         // glyph 0
                metric2.clone(), // glyph 1
                metric3,         // glyph 2
                metric4.clone(), // glyph 3
                metric5,         // glyph 4
            ],
            left_side_bearings: vec![
                6, // glyph 5
                7, // glyph 6
                8, // glyph 7,
            ],
        };

        assert_eq!(
            hmtx.subset(&[Glyph::new(1), Glyph::new(3), Glyph::new(6)], ())
                .as_ref(),
            &HmtxTable {
                h_metrics: vec![metric2, metric4],
                left_side_bearings: vec![7]
            }
        );
    }
}
