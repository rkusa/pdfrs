use std::io;

use super::hhea::HheaTable;
use super::maxp::MaxpTable;
use crate::packed::Packed;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table contains glyph metrics used for horizontal text layout.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/hmtx
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6hmtx.html
#[derive(Debug, PartialEq)]
pub struct HmtxTable {
    /// Paired advance width and left side bearing values for each glyph. Records are indexed by glyph ID.
    h_metrics: Vec<LongHorMetric>,
    /// Left side bearings for glyph IDs greater than or equal to numberOfHMetrics.
    left_side_bearings: Vec<i16>,
}

#[derive(Debug, PartialEq)]
pub struct LongHorMetric {
    /// Advance width, in font design units.
    advance_width: u16,
    /// Glyph left side bearing, in font design units.
    lsb: i16,
}

impl<'a> Packed<'a> for HmtxTable {
    type Dep = (&'a HheaTable, &'a MaxpTable);

    fn unpack<R: io::Read>(mut rd: &mut R, (hhea, maxp): Self::Dep) -> Result<Self, io::Error> {
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

    fn pack<W: io::Write>(&'a self, mut wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        // TODO: update values
        for metric in &self.h_metrics {
            metric.pack(&mut wr, ())?;
        }
        for bearing in &self.left_side_bearings {
            wr.write_i16::<BigEndian>(*bearing)?;
        }
        Ok(())
    }
}

impl<'a> Packed<'a> for LongHorMetric {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        Ok(LongHorMetric {
            advance_width: rd.read_u16::<BigEndian>()?,
            lsb: rd.read_i16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
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
        hmtx_table
            .pack(&mut buffer, (&hhea_table, &maxp_table))
            .unwrap();
        assert_eq!(
            HmtxTable::unpack(&mut Cursor::new(buffer), (&hhea_table, &maxp_table)).unwrap(),
            hmtx_table
        );
    }
}
