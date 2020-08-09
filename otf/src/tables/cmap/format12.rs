use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{self, Cursor};

use crate::tables::{FontData, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, PartialEq, Clone)]
pub struct Format12 {
    pub(crate) language: u32,
    pub(crate) num_groups: u32,
    pub(crate) sequential_map_groups: Vec<SequentialMapGroup>,
}

impl Format12 {
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        let search_result = self
            .sequential_map_groups
            .binary_search_by_key(&codepoint, |group| group.end_char_code);
        let ix = match search_result {
            // Found a direct match
            Ok(ix) => ix,
            // No direct match, `ix` represents the position where the codepoint could be inserted
            // while maintaining sorted order -> the index represents the first end code that is
            // greater than the character code
            Err(ix) => ix,
        };
        let group = self.sequential_map_groups.get(ix)?;
        if codepoint < group.start_char_code || codepoint > group.end_char_code {
            return None;
        }

        u16::try_from(
            group
                .start_glyph_id
                .checked_add(codepoint - group.start_char_code)?,
        )
        .ok()
    }
}

impl<'a> FontData<'a> for Format12 {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let language = rd.read_u32::<BigEndian>()?;
        let num_groups = rd.read_u32::<BigEndian>()?;

        let mut groups = Vec::with_capacity(num_groups as usize);
        for _ in 0..num_groups {
            groups.push(SequentialMapGroup::unpack(&mut rd, ())?);
        }

        Ok(Format12 {
            language,
            num_groups,
            sequential_map_groups: groups,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(self.language)?;
        wr.write_u32::<BigEndian>(self.sequential_map_groups.len() as u32)?;
        for group in &self.sequential_map_groups {
            group.pack(&mut wr)?;
        }
        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _dep: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        // Create a sorted Vec of (code point, new glyph index)
        let mut code_points: Vec<(u32, u32)> = glyphs
            .iter()
            .enumerate()
            .flat_map(|(new_index, g)| {
                // skip reserved index 0 (reserved for default glyph)
                let new_index = new_index + 1;
                g.code_points.iter().filter_map(move |c| {
                    u32::try_from(new_index)
                        .ok()
                        .map(|new_index| (*c, new_index))
                })
            })
            .collect();
        code_points.sort_by_key(|(c, _)| *c);

        let segments = code_points
            .into_iter()
            .map(|(c, i)| SequentialMapGroup {
                start_char_code: c,
                end_char_code: c,
                start_glyph_id: i,
            })
            .collect::<Vec<_>>();

        // merge adjacent segments
        let mut segments: Vec<SequentialMapGroup> =
            segments.into_iter().fold(Vec::new(), |mut segments, s| {
                if let Some(prev) = segments.last_mut() {
                    if prev.end_char_code.saturating_add(1) == s.start_char_code
                        && (prev.start_glyph_id + (s.start_char_code - prev.start_char_code))
                            == s.start_glyph_id
                    {
                        prev.end_char_code = s.start_char_code;
                        return segments;
                    }
                }
                segments.push(s);
                segments
            });

        if segments.len() > u32::MAX as usize {
            segments.resize_with(u32::MAX as usize, Default::default)
        }

        Cow::Owned(Format12 {
            language: self.language,
            num_groups: u32::try_from(segments.len()).ok().unwrap_or(u32::MAX),
            sequential_map_groups: segments,
        })
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct SequentialMapGroup {
    start_char_code: u32,
    end_char_code: u32,
    start_glyph_id: u32,
}

impl<'a> FontData<'a> for SequentialMapGroup {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        Ok(SequentialMapGroup {
            start_char_code: rd.read_u32::<BigEndian>()?,
            end_char_code: rd.read_u32::<BigEndian>()?,
            start_glyph_id: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u32::<BigEndian>(self.start_char_code)?;
        wr.write_u32::<BigEndian>(self.end_char_code)?;
        wr.write_u32::<BigEndian>(self.start_glyph_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use super::*;
    use crate::tables::cmap::{CmapTable, Subtable};
    use crate::OffsetTable;

    fn get_format12_subtable() -> Format12 {
        let data = include_bytes!("../../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let cmap_table: CmapTable = table.unpack_required_table((), &mut cursor).unwrap();

        let record = cmap_table
            .encoding_records
            .into_iter()
            .find(|r| r.platform_id == 0 && r.encoding_id == 4)
            .unwrap();

        match Rc::try_unwrap(record.subtable).unwrap() {
            Subtable::Format12(subtable) => subtable,
            _ => panic!("Expected format 12 subtable"),
        }
    }

    #[test]
    fn test_cmap_subtable_format12_encode_decode() {
        let format12 = get_format12_subtable();

        assert_eq!(
            format12.sequential_map_groups.len(),
            format12.num_groups as usize
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        format12.pack(&mut buffer).unwrap();
        assert_eq!(
            Format12::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            format12
        );
    }

    #[test]
    fn test_cmap_subtable_format12_codepoint_to_glyph_id() {
        let format12 = get_format12_subtable();

        assert_eq!(format12.glyph_id(12), None);
        assert_eq!(format12.glyph_id(13), Some(2));
        assert_eq!(format12.glyph_id(422), Some(360));
        assert_eq!(format12.glyph_id(8694), None);
        assert_eq!(format12.glyph_id(129989), Some(3557));
        assert_eq!(format12.glyph_id(130041), Some(3572));
        assert_eq!(format12.glyph_id(130042), None);
    }

    #[test]
    fn test_cmap_subtable_format12_subset() {
        let format12 = get_format12_subtable();

        let glyphs = &['a', 'b', '₨', '❶', '❷', '❸', 'ɸ']
            .iter()
            .map(|c| Glyph {
                index: format12.glyph_id(u32::from(*c)).unwrap(),
                code_points: vec![u32::from(*c)],
            })
            .chain(std::iter::once(Glyph {
                index: 40,
                code_points: vec![3],
            }))
            .collect::<Vec<_>>();
        let subset = format12.subset(&glyphs, ());

        assert_eq!(subset.sequential_map_groups.len(), 5);

        for (i, g) in glyphs.iter().enumerate() {
            for c in &g.code_points {
                // i + 1, since 0 should be the missing glyph
                assert_eq!(subset.glyph_id(*c), Some((i + 1) as u16));
            }
        }

        assert_eq!(subset.glyph_id(0), None);
        assert_eq!(subset.glyph_id(u16::MAX as u32), None);
    }
}
