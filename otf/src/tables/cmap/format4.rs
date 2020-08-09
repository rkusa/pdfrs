use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{self, Cursor, Read};
use std::{iter, mem};

use crate::tables::{FontData, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, PartialEq, Clone)]
pub struct Format4 {
    /// Should always be `0` since this library does not parse Macintosh CMAPs.
    pub(crate) language: u16,
    /// 2 × seg_count.
    pub(crate) seg_count_x2: u16,
    /// 2 × (2**floor(log2(seg_count)))
    pub(crate) search_range: u16,
    /// log2(search_range/2)
    pub(crate) entry_selector: u16,
    /// 2 × seg_count - search_range
    pub(crate) range_shift: u16,
    /// End characterCode for each segment, last=0xFFFF.
    pub(crate) end_code: Vec<u16>,
    /// Set to 0.
    pub(crate) reserved_pad: u16,
    /// Start character code for each segment.
    pub(crate) start_code: Vec<u16>,
    /// Delta for all character codes in segment.
    pub(crate) id_delta: Vec<i16>,
    /// Offsets into glyph_id_array or 0
    pub(crate) id_range_offset: Vec<u16>,
    /// Glyph index array (arbitrary length)
    pub(crate) glyph_id_array: Vec<u8>,
}

impl Format4 {
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        // Return None for codepoints > `u16::MAX`
        let codepoint = u16::try_from(codepoint).ok()?;

        // Search for the first end_code that is greater than or equal to the character code
        let ix = match self.end_code.binary_search(&codepoint) {
            // Found a direct match
            Ok(ix) => ix,
            // No direct match, `ix` represents the position where the codepoint could be inserted
            // while maintaining sorted order -> the index represents the first end code that is
            // greater than the character code
            Err(ix) => ix,
        };
        let start_code = *self.start_code.get(ix)?;

        if start_code <= codepoint {
            let id_range_offset = *self.id_range_offset.get(ix)?;
            let id_delta = *self.id_delta.get(ix)?;
            let val = if id_range_offset == 0 {
                codepoint
            } else {
                // id_range_offset + (codepoint - start_code) * 2 + 2 * ix
                let pos = id_range_offset
                    .checked_add(codepoint.checked_sub(start_code)?.checked_mul(2)?)?
                    .checked_add(
                        (mem::size_of::<u16>() as u16).checked_mul(u16::try_from(ix).ok()?)?,
                    )? as usize;

                if pos / 2 < self.id_range_offset.len() {
                    self.id_range_offset.get(pos / 2).cloned()
                } else {
                    let offset = pos - self.id_range_offset.len() * 2;
                    (&self.glyph_id_array[offset..])
                        .read_u16::<BigEndian>()
                        .ok()
                }
                .filter(|v| *v != 0)?
            };
            Some(u16::try_from((val as i32) + id_delta as i32).ok()?)
        } else {
            None
        }
    }
}

impl<'a> FontData<'a> for Format4 {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let language = rd.read_u16::<BigEndian>()?;
        let seg_count_x2 = rd.read_u16::<BigEndian>()?;
        let seg_count = (seg_count_x2 / 2) as usize;
        let search_range = rd.read_u16::<BigEndian>()?;
        let entry_selector = rd.read_u16::<BigEndian>()?;
        let range_shift = rd.read_u16::<BigEndian>()?;
        let mut end_code = vec![0; seg_count];
        rd.read_u16_into::<BigEndian>(&mut end_code)?;
        let reserved_pad = rd.read_u16::<BigEndian>()?;
        let mut start_code = vec![0; seg_count];
        rd.read_u16_into::<BigEndian>(&mut start_code)?;
        let mut id_delta = vec![0; seg_count];
        rd.read_i16_into::<BigEndian>(&mut id_delta)?;

        let mut id_range_offset = vec![0; seg_count];
        rd.read_u16_into::<BigEndian>(&mut id_range_offset)?;

        let mut glyph_id_array = Vec::new();
        rd.read_to_end(&mut glyph_id_array)?;

        Ok(Format4 {
            language,
            seg_count_x2,
            search_range,
            entry_selector,
            range_shift,
            end_code,
            reserved_pad,
            start_code,
            id_delta,
            id_range_offset,
            glyph_id_array,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.language)?;
        wr.write_u16::<BigEndian>(self.seg_count_x2)?;
        wr.write_u16::<BigEndian>(self.search_range)?;
        wr.write_u16::<BigEndian>(self.entry_selector)?;
        wr.write_u16::<BigEndian>(self.range_shift)?;
        for end_code in &self.end_code {
            wr.write_u16::<BigEndian>(*end_code)?;
        }
        wr.write_u16::<BigEndian>(0)?;
        for start_code in &self.start_code {
            wr.write_u16::<BigEndian>(*start_code)?;
        }
        for id_delta in &self.id_delta {
            wr.write_i16::<BigEndian>(*id_delta)?;
        }
        for range_offset in &self.id_range_offset {
            wr.write_u16::<BigEndian>(*range_offset)?;
        }
        wr.write_all(&self.glyph_id_array)?;
        Ok(())
    }

    /// Create a subset of the Format 4 CMAP table for the given `glyphs`.
    /// Note: All code points > `u16::MAX` are simply ignored.
    fn subset(&'a self, glyphs: &[Glyph], _dep: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        // Create a sorted Vec of (code point, new glyph index)
        let mut code_points: Vec<(u16, u16)> = glyphs
            .iter()
            .enumerate()
            .flat_map(|(new_index, g)| {
                // skip reserved index 0 (reserved for default glyph)
                let new_index = new_index + 1;
                g.code_points.iter().filter_map(move |c| {
                    u16::try_from(new_index)
                        .ok()
                        .and_then(|new_index| u16::try_from(*c).ok().map(|c| (c, new_index)))
                })
            })
            .collect();
        code_points.sort_by_key(|(c, _)| *c);

        #[derive(Debug)]
        struct Segment {
            start: u16,
            end: u16,
            id_delta: i16,
        }

        let segments = code_points
            .into_iter()
            .filter_map(|(c, i)| {
                let (d, wrapped) = i.overflowing_sub(c);
                (if wrapped {
                    i16::try_from(u16::MAX - d)
                        .ok()
                        .and_then(|d| d.checked_add(1))
                        .and_then(|d| d.checked_mul(-1))
                } else {
                    i16::try_from(d).ok()
                })
                .map(|id_delta| Segment {
                    start: c,
                    end: c,
                    id_delta,
                })
            })
            // End segment must always exist
            .chain(iter::once(Segment {
                start: u16::MAX,
                end: u16::MAX,
                id_delta: 0,
            }))
            .collect::<Vec<_>>();

        // merge adjacent segments
        let segments: Vec<Segment> = segments.into_iter().fold(Vec::new(), |mut segments, s| {
            if let Some(prev) = segments.last_mut() {
                if prev.id_delta == s.id_delta && prev.end.saturating_add(1) == s.start {
                    prev.end = s.start;
                    return segments;
                }
            }
            segments.push(s);
            segments
        });

        let mut id_range_offset: Vec<u16> = vec![0; segments.len()];
        let glyph_id_array = vec![0; 2]; // 0u16

        // add missing glyph index `0` for last segment
        // id_range_offset = pos - 2 * ix
        let pos = id_range_offset.len() * 2;
        let ix = segments.len() - 1;
        id_range_offset[segments.len() - 1] = u16::try_from(pos - 2 * ix).ok().unwrap_or(0);

        let seg_count_x2 = u16::try_from(segments.len().saturating_mul(2)).unwrap_or(0);
        let search_range = 2 * 2u16.pow(((self.seg_count_x2 / 2) as f32).log2().floor() as u32);
        let entry_selector = (search_range as f32 / 2.0).log2() as u16;
        let range_shift = self.seg_count_x2 - search_range;

        Cow::Owned(Format4 {
            language: self.language,
            seg_count_x2,
            search_range,
            entry_selector,
            range_shift,
            end_code: segments.iter().map(|s| s.end).collect(),
            reserved_pad: self.reserved_pad,
            start_code: segments.iter().map(|s| s.start).collect(),
            id_delta: segments.iter().map(|s| s.id_delta).collect(),
            id_range_offset,
            glyph_id_array,
        })
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use super::*;
    use crate::tables::cmap::{CmapTable, Subtable};
    use crate::OffsetTable;

    fn get_format4_subtable() -> Format4 {
        let data = include_bytes!("../../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let cmap_table: CmapTable = table.unpack_required_table((), &mut cursor).unwrap();

        let record = cmap_table
            .encoding_records
            .into_iter()
            .find(|r| r.platform_id == 0 && r.encoding_id == 3)
            .unwrap();

        match Rc::try_unwrap(record.subtable).unwrap() {
            Subtable::Format4(subtable) => subtable,
            _ => panic!("Expected format 4 subtable"),
        }
    }

    #[test]
    fn test_cmap_subtable_format4_encode_decode() {
        let format4 = get_format4_subtable();

        let seg_count = format4.seg_count_x2 as usize / 2;
        assert_eq!(format4.end_code.len(), seg_count);
        assert_eq!(format4.start_code.len(), seg_count);
        assert_eq!(format4.id_delta.len(), seg_count);
        assert_eq!(format4.id_range_offset.len(), seg_count);

        // re-pack and compare
        let mut buffer = Vec::new();
        format4.pack(&mut buffer).unwrap();
        assert_eq!(
            Format4::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            format4
        );
    }

    #[test]
    fn test_cmap_subtable_format4_codepoint_to_glyph_id_without_range_offset() {
        let format4 = Format4 {
            language: 0,
            seg_count_x2: 8,
            search_range: 8,
            entry_selector: 4,
            range_shift: 0,
            end_code: vec![20, 90, 480, 0xFFFF],
            reserved_pad: 0,
            start_code: vec![10, 30, 153, 0xFFFF],
            id_delta: vec![-9, -18, -27, 1],
            id_range_offset: vec![0, 0, 0, 0],
            glyph_id_array: Vec::new(),
        };

        assert_eq!(format4.glyph_id(10), Some(1));
        assert_eq!(format4.glyph_id(20), Some(11));
        assert_eq!(format4.glyph_id(30), Some(12));
        assert_eq!(format4.glyph_id(90), Some(72));
    }

    #[test]
    fn test_cmap_subtable_format4_codepoint_to_glyph_id_with_range_offset() {
        let format4 = Format4 {
            language: 0,
            seg_count_x2: 4,
            search_range: 4,
            entry_selector: 1,
            range_shift: 0,
            end_code: vec![12, 0xFFFF],
            reserved_pad: 0,
            start_code: vec![10, 0xFFFF],
            id_delta: vec![0, 1],
            id_range_offset: vec![4, 0],
            glyph_id_array: vec![
                0x00, 0x01, // glyph_id_array 0: 1
                0x00, 0x02, // glyph_id_array 1: 2
                0x00, 0x03, // glyph_id_array 2: 3
            ],
        };

        assert_eq!(format4.glyph_id(0), None);
        assert_eq!(format4.glyph_id(10), Some(1));
        assert_eq!(format4.glyph_id(11), Some(2));
        assert_eq!(format4.glyph_id(12), Some(3));
        assert_eq!(format4.glyph_id(13), None);

        // re-pack and compare
        let mut buffer = Vec::new();
        format4.pack(&mut buffer).unwrap();
        assert_eq!(
            Format4::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            format4
        );
    }

    #[test]
    fn test_cmap_subtable_format4_subset() {
        let format4 = get_format4_subtable();

        let glyphs = &['a', 'b', '₨', '❶', '❷', '❸', 'ɸ']
            .iter()
            .map(|c| Glyph {
                index: format4.glyph_id(u32::from(*c)).unwrap(),
                code_points: vec![u32::from(*c)],
            })
            .chain(std::iter::once(Glyph {
                index: 40,
                code_points: vec![3],
            }))
            .collect::<Vec<_>>();
        let subset = format4.subset(&glyphs, ());

        assert_eq!(subset.start_code.len(), 6);

        for (i, g) in glyphs.iter().enumerate() {
            for c in &g.code_points {
                // i + 1, since 0 should be the missing glyph
                assert_eq!(subset.glyph_id(*c), Some((i + 1) as u16));
            }
        }

        assert_eq!(subset.glyph_id(0), None);
        assert_eq!(subset.glyph_id(u16::MAX as u32), None);

        // should update header
        assert_eq!(subset.seg_count_x2, 12);
        assert_eq!(subset.search_range, 256);
        assert_eq!(subset.entry_selector, 7);
        assert_eq!(subset.range_shift, 166);
    }
}
