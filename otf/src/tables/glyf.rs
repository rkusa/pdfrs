use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Cursor, Read, Write};
use std::{iter, mem};

use super::loca::LocaTable;
use super::{FontData, FontTable, Glyph};
use crate::utils::align_write::AlignWrite;
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// The 'glyf' table is comprised of a list of glyph data blocks, each of which provides the
/// description for a single glyph. Glyphs are referenced by identifiers (glyph IDs), which are
/// sequential integers beginning at zero.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/glyf
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6glyf.html
#[derive(Debug, PartialEq, Clone)]
pub struct GlyfTable {
    pub(crate) glyphs: Vec<Option<GlyphData>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct GlyphData {
    /// If the number of contours is greater than or equal to zero, this is a simple glyph. If
    /// negative, this is a composite glyph — the value -1 should be used for composite glyphs.
    pub(crate) number_of_contours: i16,
    /// Minimum x for coordinate data.
    pub(crate) x_min: i16,
    /// Minimum y for coordinate data.
    pub(crate) y_min: i16,
    /// Maximum x for coordinate data.
    pub(crate) x_max: i16,
    /// Maximum y for coordinate data.
    pub(crate) y_max: i16,
    /// The glyph description.
    pub(crate) description: GlyphDescription,
}

#[derive(Debug, PartialEq, Clone)]
pub enum GlyphDescription {
    // TODO: parse simple glyph
    Simple(Vec<u8>),
    Composite(CompositeDescription),
}

#[derive(Debug, PartialEq, Clone)]
pub struct CompositeDescription {
    components: Vec<Component>,
    instructions: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Component {
    flags: u16,
    glyph_index: u16,
    args: Args,
    scale: Option<Scale>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Args {
    U16(u16, u16),
    I16(i16, i16),
    U8(u8, u8),
    I8(i8, i8),
}

#[derive(Debug, PartialEq, Clone)]
// TODO: values are actually F2DOT14
pub enum Scale {
    Simple(i16),
    XY {
        x: i16,
        y: i16,
    },
    TwoByTwo {
        x: i16,
        scale01: i16,
        scale10: i16,
        y: i16,
    },
}

impl GlyfTable {
    pub(crate) fn expand_composite_glyphs(&self, glyphs: &[Glyph]) -> Vec<Glyph> {
        // collect all composite glyph components
        let mut visited: HashSet<u16> = HashSet::new();
        let mut ordered = Vec::new();

        // Always include glyph index 0, since this is supposed to be the default glyph
        let mut all_glyphs: VecDeque<u16> = iter::once(0)
            .chain(glyphs.iter().map(|g| g.index))
            .collect();

        while let Some(ix) = all_glyphs.pop_front() {
            if visited.contains(&ix) {
                continue;
            }

            if let Some(Some(g)) = self.glyphs.get(ix as usize) {
                if let GlyphDescription::Composite(composite) = &g.description {
                    for component in &composite.components {
                        if !visited.contains(&component.glyph_index) {
                            all_glyphs.push_back(component.glyph_index);
                        }
                    }
                }
            }

            visited.insert(ix);
            ordered.push(ix);
        }

        glyphs
            .iter()
            .cloned()
            .chain(ordered.into_iter().skip(glyphs.len()).map(|index| Glyph {
                index,
                code_points: Vec::new(),
            }))
            .collect()
    }
}

impl<'a> FontTable<'a, &'a LocaTable, ()> for GlyfTable {
    fn name() -> &'static str {
        "glyf"
    }
}

impl GlyphData {
    pub fn size_in_byte(&self) -> usize {
        let mut size = mem::size_of::<i16>() * 5 + self.description.size_in_byte();
        // aligned to 4 bytes
        if size % 4 != 0 {
            size += 4 - (size % 4)
        }
        size
    }
}

impl GlyphDescription {
    pub fn size_in_byte(&self) -> usize {
        match self {
            GlyphDescription::Simple(data) => data.len(),
            GlyphDescription::Composite(composite) => composite.size_in_byte(),
        }
    }
}

impl CompositeDescription {
    pub fn size_in_byte(&self) -> usize {
        self.components
            .iter()
            .map(|c| c.size_in_byte())
            .sum::<usize>()
            + self
                .instructions
                .as_ref()
                .map(|i| i.len() + mem::size_of::<u16>())
                .unwrap_or(0)
    }
}

impl Component {
    pub fn size_in_byte(&self) -> usize {
        mem::size_of::<u16>() * 2
            + self.args.size_in_byte()
            + self.scale.as_ref().map(|s| s.size_in_byte()).unwrap_or(0)
    }
}

impl Args {
    pub fn size_in_byte(&self) -> usize {
        match self {
            Args::U16(_, _) => 2 * mem::size_of::<u16>(),
            Args::I16(_, _) => 2 * mem::size_of::<i16>(),
            Args::U8(_, _) => 2 * mem::size_of::<u8>(),
            Args::I8(_, _) => 2 * mem::size_of::<i8>(),
        }
    }
}

impl Scale {
    pub fn size_in_byte(&self) -> usize {
        match self {
            Scale::Simple(_) => mem::size_of::<i16>(),
            Scale::XY { .. } => 2 * mem::size_of::<i16>(),
            Scale::TwoByTwo { .. } => 4 * mem::size_of::<i16>(),
        }
    }
}

const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
// const ROUND_XY_TO_GRID: u16 = 0x0004;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;
// const USE_MY_METRICS: u16 = 0x0200;
// const OVERLAP_COMPOUND: u16 = 0x0400;
// const SCALED_COMPONENT_OFFSET: u16 = 0x0800;
// const UNSCALED_COMPONENT_OFFSET: u16 = 0x1000;

impl<'a> FontData<'a> for GlyfTable {
    type UnpackDep = &'a LocaTable;
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        loca: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let mut glyphs = Vec::with_capacity(loca.offsets.len().saturating_sub(1));

        let mut pos = 0;
        for (start, end) in loca.offsets.iter().zip(loca.offsets.iter().skip(1)) {
            let start = *start as usize;
            let end = *end as usize;

            if start == end {
                // glyph has no outline
                glyphs.push(None);
                continue;
            }

            if start > pos {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Encountered unaligned LOCA table offsets",
                ));
            }

            rd.set_position(start as u64);
            let mut lrd = Cursor::new(LimitRead::from_cursor(rd, end - start));
            glyphs.push(Some(GlyphData::unpack(&mut lrd, ())?));

            // Discarding the a possible 4-bytes allignment remainder.
            lrd.into_inner().discard()?;

            pos = end;
        }

        Ok(GlyfTable { glyphs })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        for data in &self.glyphs {
            if let Some(data) = data {
                data.pack(&mut wr)?;
            }
        }
        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _dep: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        let old_to_new: HashMap<u16, u16> = glyphs
            .iter()
            .enumerate()
            .map(|(i, g)| (g.index, i as u16))
            .collect();
        Cow::Owned(GlyfTable {
            glyphs: glyphs
                .iter()
                .map(|g| {
                    self.glyphs
                        .get(g.index as usize)
                        .cloned()
                        .flatten()
                        .map(|mut g| {
                            if let GlyphDescription::Composite(ref mut composite) =
                                &mut g.description
                            {
                                for component in &mut composite.components {
                                    component.glyph_index = old_to_new
                                        .get(&component.glyph_index)
                                        .cloned()
                                        .unwrap_or(0);
                                }
                            }
                            g
                        })
                })
                .collect(),
        })
    }
}

impl<'a> FontData<'a> for GlyphData {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let number_of_contours = rd.read_i16::<BigEndian>()?;
        let x_min = rd.read_i16::<BigEndian>()?;
        let y_min = rd.read_i16::<BigEndian>()?;
        let x_max = rd.read_i16::<BigEndian>()?;
        let y_max = rd.read_i16::<BigEndian>()?;

        let description = if number_of_contours < 0 {
            let mut components = Vec::with_capacity(1);

            let mut has_more = true;
            let mut flags = 0;
            let mut lc = 0;
            while has_more {
                lc += 1;
                assert!(lc < 10);

                flags = rd.read_u16::<BigEndian>()?;
                let glyph_index = rd.read_u16::<BigEndian>()?;

                #[allow(clippy::collapsible_if)]
                let args = if flags & ARG_1_AND_2_ARE_WORDS != 0 {
                    if flags & ARGS_ARE_XY_VALUES != 0 {
                        Args::I16(rd.read_i16::<BigEndian>()?, rd.read_i16::<BigEndian>()?)
                    } else {
                        Args::U16(rd.read_u16::<BigEndian>()?, rd.read_u16::<BigEndian>()?)
                    }
                } else {
                    if flags & ARGS_ARE_XY_VALUES != 0 {
                        Args::I8(rd.read_i8()?, rd.read_i8()?)
                    } else {
                        Args::U8(rd.read_u8()?, rd.read_u8()?)
                    }
                };

                let scale = if flags & WE_HAVE_A_SCALE != 0 {
                    Some(Scale::Simple(rd.read_i16::<BigEndian>()?))
                } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
                    Some(Scale::XY {
                        x: rd.read_i16::<BigEndian>()?,
                        y: rd.read_i16::<BigEndian>()?,
                    })
                } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
                    Some(Scale::TwoByTwo {
                        x: rd.read_i16::<BigEndian>()?,
                        scale01: rd.read_i16::<BigEndian>()?,
                        scale10: rd.read_i16::<BigEndian>()?,
                        y: rd.read_i16::<BigEndian>()?,
                    })
                } else {
                    None
                };

                components.push(Component {
                    flags,
                    glyph_index,
                    args,
                    scale,
                });
                has_more = flags & MORE_COMPONENTS != 0;
            }

            let instructions = if flags & WE_HAVE_INSTRUCTIONS != 0 {
                let len = rd.read_u16::<BigEndian>()? as usize;
                let mut instructions = vec![0; len];
                rd.read_exact(&mut instructions)?;
                Some(instructions)
            } else {
                None
            };

            GlyphDescription::Composite(CompositeDescription {
                components,
                instructions,
            })
        } else {
            let mut description = Vec::new();
            rd.read_to_end(&mut description)?;
            GlyphDescription::Simple(description)
        };

        Ok(GlyphData {
            number_of_contours,
            x_min,
            y_min,
            x_max,
            y_max,
            description,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        let mut awr = AlignWrite::new(&mut wr, 4);

        awr.write_i16::<BigEndian>(self.number_of_contours)?;
        awr.write_i16::<BigEndian>(self.x_min)?;
        awr.write_i16::<BigEndian>(self.y_min)?;
        awr.write_i16::<BigEndian>(self.x_max)?;
        awr.write_i16::<BigEndian>(self.y_max)?;

        match &self.description {
            GlyphDescription::Simple(desc) => awr.write_all(&desc)?,
            GlyphDescription::Composite(CompositeDescription {
                components,
                instructions,
            }) => {
                for (i, component) in components.iter().enumerate() {
                    let mut flags = component.flags;
                    match component.args {
                        Args::I16(_, _) | Args::U16(_, _) => flags |= ARG_1_AND_2_ARE_WORDS,
                        Args::I8(_, _) | Args::U8(_, _) => flags &= !ARG_1_AND_2_ARE_WORDS,
                    }
                    match component.args {
                        Args::I16(_, _) | Args::I8(_, _) => flags |= ARGS_ARE_XY_VALUES,
                        Args::U16(_, _) | Args::U8(_, _) => flags &= !ARGS_ARE_XY_VALUES,
                    }
                    flags &= !WE_HAVE_A_SCALE;
                    flags &= !WE_HAVE_AN_X_AND_Y_SCALE;
                    flags &= !WE_HAVE_A_TWO_BY_TWO;
                    match component.scale {
                        Some(Scale::Simple(_)) => flags |= WE_HAVE_A_SCALE,
                        Some(Scale::XY { .. }) => flags |= WE_HAVE_AN_X_AND_Y_SCALE,
                        Some(Scale::TwoByTwo { .. }) => flags |= WE_HAVE_A_TWO_BY_TWO,
                        _ => {}
                    }

                    let is_last = components.len() == i + 1;
                    if !is_last {
                        flags |= MORE_COMPONENTS;
                    }

                    if is_last && instructions.is_some() {
                        flags |= WE_HAVE_INSTRUCTIONS;
                    } else {
                        flags &= !WE_HAVE_INSTRUCTIONS;
                    }

                    awr.write_u16::<BigEndian>(flags)?;
                    awr.write_u16::<BigEndian>(component.glyph_index)?;
                    match component.args {
                        Args::I16(a, b) => {
                            awr.write_i16::<BigEndian>(a)?;
                            awr.write_i16::<BigEndian>(b)?;
                        }
                        Args::U16(a, b) => {
                            awr.write_u16::<BigEndian>(a)?;
                            awr.write_u16::<BigEndian>(b)?;
                        }
                        Args::I8(a, b) => {
                            awr.write_i8(a)?;
                            awr.write_i8(b)?;
                        }
                        Args::U8(a, b) => {
                            awr.write_u8(a)?;
                            awr.write_u8(b)?;
                        }
                    }

                    match component.scale {
                        Some(Scale::Simple(s)) => awr.write_i16::<BigEndian>(s)?,
                        Some(Scale::XY { x, y }) => {
                            awr.write_i16::<BigEndian>(x)?;
                            awr.write_i16::<BigEndian>(y)?;
                        }
                        Some(Scale::TwoByTwo {
                            x,
                            scale01,
                            scale10,
                            y,
                        }) => {
                            awr.write_i16::<BigEndian>(x)?;
                            awr.write_i16::<BigEndian>(scale01)?;
                            awr.write_i16::<BigEndian>(scale10)?;
                            awr.write_i16::<BigEndian>(y)?;
                        }
                        _ => {}
                    }
                }

                if let Some(instructions) = instructions {
                    awr.write_u16::<BigEndian>(instructions.len() as u16)?;
                    awr.write_all(&instructions)?;
                }
            }
        }

        let written = awr.end_aligned()?;
        assert_eq!(written, self.size_in_byte());

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tables::head::HeadTable;
    use crate::tables::maxp::MaxpTable;
    use crate::OffsetTable;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_simple_glyph_size_in_bytes() {
        let g = GlyphData {
            number_of_contours: 1,
            x_min: 1,
            y_min: 1,
            x_max: 1,
            y_max: 1,
            description: GlyphDescription::Simple(vec![0; 10]),
        };
        assert_eq!(g.size_in_byte(), 20);
    }

    #[test]
    fn test_composite_glyph_size_in_bytes() {
        let g = GlyphData {
            number_of_contours: 1,
            x_min: 1,
            y_min: 1,
            x_max: 1,
            y_max: 1,
            description: GlyphDescription::Composite(CompositeDescription {
                components: vec![
                    Component {
                        flags: 0,
                        glyph_index: 0,
                        args: Args::U16(1, 2),
                        scale: Some(Scale::XY { x: 3, y: 4 }),
                    },
                    Component {
                        flags: 0,
                        glyph_index: 0,
                        args: Args::I8(1, 2),
                        scale: Some(Scale::Simple(3)),
                    },
                ],
                instructions: Some(vec![0; 10]),
            }),
        };
        assert_eq!(g.size_in_byte(), 44);
    }

    #[test]
    fn test_glyf_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let head_table: HeadTable = table.unpack_required_table((), &mut cursor).unwrap();
        let maxp_table: MaxpTable = table.unpack_required_table((), &mut cursor).unwrap();
        let loca_table: LocaTable = table
            .unpack_required_table((&head_table, &maxp_table), &mut cursor)
            .unwrap();
        let glyf_table: GlyfTable = table
            .unpack_required_table(&loca_table, &mut cursor)
            .unwrap();

        assert_eq!(
            glyf_table.glyphs.len(),
            (loca_table.offsets.len() as usize) - 1
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        glyf_table.pack(&mut buffer).unwrap();

        let new_table = GlyfTable::unpack(&mut Cursor::new(&buffer[..]), &loca_table).unwrap();
        assert_eq!(new_table.glyphs.len(), glyf_table.glyphs.len());
        for (i, (l, r)) in new_table.glyphs.iter().zip(&glyf_table.glyphs).enumerate() {
            assert_eq!(l, r, "Glyphs at index {} don't matchs", i);
        }
    }

    #[test]
    fn test_glyf_table_subset() {
        let g0 = GlyphData {
            number_of_contours: 0,
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
            description: GlyphDescription::Simple(Vec::new()),
        };
        let g1 = GlyphData {
            number_of_contours: 1,
            x_min: 1,
            y_min: 1,
            x_max: 1,
            y_max: 1,
            description: GlyphDescription::Simple(Vec::new()),
        };
        let g2 = GlyphData {
            number_of_contours: 2,
            x_min: 2,
            y_min: 2,
            x_max: 2,
            y_max: 2,
            description: GlyphDescription::Simple(Vec::new()),
        };
        let g3 = GlyphData {
            number_of_contours: 3,
            x_min: 3,
            y_min: 3,
            x_max: 3,
            y_max: 3,
            description: GlyphDescription::Simple(Vec::new()),
        };

        let table = GlyfTable {
            glyphs: vec![Some(g0), Some(g1), Some(g2.clone()), Some(g3), None],
        };
        assert_eq!(
            table.subset(&[Glyph::new(2), Glyph::new(4)], ()).as_ref(),
            &GlyfTable {
                glyphs: vec![Some(g2), None]
            }
        )
    }
}
