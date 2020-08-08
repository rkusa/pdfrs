use std::borrow::Cow;
use std::convert::TryFrom;
use std::io;

use super::{FontTable, Glyph};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table establishes the memory requirements for this font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/maxp
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6maxp.html
#[derive(Debug, PartialEq, Clone)]
pub enum MaxpTable {
    // Version 0.5
    CFF(CffMaxpTable),
    // Version 1.0
    TrueType(TrueTypeMaxpTable),
}

#[derive(Debug, PartialEq, Clone)]
pub struct CffMaxpTable {
    /// The number of glyphs in the font.
    num_glyphs: u16,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TrueTypeMaxpTable {
    /// The number of glyphs in the font.
    num_glyphs: u16,
    /// Maximum points in a non-composite glyph.
    max_points: u16,
    /// Maximum contours in a non-composite glyph.
    max_contours: u16,
    /// Maximum points in a composite glyph.
    max_component_points: u16,
    /// Maximum contours in a composite glyph.
    max_component_contours: u16,
    /// 1 if instructions do not use the twilight zone (Z0), or 2 if instructions do use Z0; should be
    /// set to 2 in most cases.
    max_zones: u16,
    /// Maximum points used in Z0.
    max_twilight_points: u16,
    /// Number of Storage Area locations.
    max_storage: u16,
    /// Number of FDEFs, equal to the highest function number + 1.
    max_function_defs: u16,
    /// Number of IDEFs.
    max_instruction_defs: u16,
    /// Maximum stack depth across Font Program ('fpgm' table), CVT Program ('prep' table) and all glyph
    /// instructions (in the 'glyf' table).
    max_stack_elements: u16,
    /// Maximum byte count for glyph instructions.
    max_size_of_instructions: u16,
    /// Maximum number of components referenced at “top level” for any composite glyph.
    max_component_elements: u16,
    /// Maximum levels of recursion; 1 for simple components.
    max_component_depth: u16,
}

impl MaxpTable {
    pub fn num_glyphs(&self) -> u16 {
        match self {
            MaxpTable::CFF(table) => table.num_glyphs,
            MaxpTable::TrueType(table) => table.num_glyphs,
        }
    }
}

impl<'a> FontTable<'a> for MaxpTable {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
        let version = rd.read_u32::<BigEndian>()?;
        match version {
            0x00005000 => Ok(MaxpTable::CFF(CffMaxpTable::unpack(&mut rd, ())?)),
            0x00010000 => Ok(MaxpTable::TrueType(TrueTypeMaxpTable::unpack(&mut rd, ())?)),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Invalid MAXP version {}", version),
            )),
        }
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W) -> Result<(), io::Error> {
        match self {
            MaxpTable::CFF(table) => {
                // version
                wr.write_u32::<BigEndian>(0x00005000)?;
                table.pack(&mut wr)?;
            }
            MaxpTable::TrueType(table) => {
                // version
                wr.write_u32::<BigEndian>(0x00010000)?;
                table.pack(&mut wr)?;
            }
        }

        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        match self {
            MaxpTable::CFF(table) => match table.subset(glyphs, ()) {
                Cow::Borrowed(_) => Cow::Borrowed(self),
                Cow::Owned(table) => Cow::Owned(MaxpTable::CFF(table)),
            },
            MaxpTable::TrueType(table) => match table.subset(glyphs, ()) {
                Cow::Borrowed(_) => Cow::Borrowed(self),
                Cow::Owned(table) => Cow::Owned(MaxpTable::TrueType(table)),
            },
        }
    }
}

impl<'a> FontTable<'a> for CffMaxpTable {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
        Ok(CffMaxpTable {
            num_glyphs: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.num_glyphs)?;
        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Owned(CffMaxpTable {
            // +1 since glyph 0 is always additionally added
            num_glyphs: u16::try_from(glyphs.len() + 1).ok().unwrap_or(u16::MAX),
        })
    }
}

impl<'a> FontTable<'a> for TrueTypeMaxpTable {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
        Ok(TrueTypeMaxpTable {
            num_glyphs: rd.read_u16::<BigEndian>()?,
            max_points: rd.read_u16::<BigEndian>()?,
            max_contours: rd.read_u16::<BigEndian>()?,
            max_component_points: rd.read_u16::<BigEndian>()?,
            max_component_contours: rd.read_u16::<BigEndian>()?,
            max_zones: rd.read_u16::<BigEndian>()?,
            max_twilight_points: rd.read_u16::<BigEndian>()?,
            max_storage: rd.read_u16::<BigEndian>()?,
            max_function_defs: rd.read_u16::<BigEndian>()?,
            max_instruction_defs: rd.read_u16::<BigEndian>()?,
            max_stack_elements: rd.read_u16::<BigEndian>()?,
            max_size_of_instructions: rd.read_u16::<BigEndian>()?,
            max_component_elements: rd.read_u16::<BigEndian>()?,
            max_component_depth: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.num_glyphs)?;
        wr.write_u16::<BigEndian>(self.max_points)?;
        wr.write_u16::<BigEndian>(self.max_contours)?;
        wr.write_u16::<BigEndian>(self.max_component_points)?;
        wr.write_u16::<BigEndian>(self.max_component_contours)?;
        wr.write_u16::<BigEndian>(self.max_zones)?;
        wr.write_u16::<BigEndian>(self.max_twilight_points)?;
        wr.write_u16::<BigEndian>(self.max_storage)?;
        wr.write_u16::<BigEndian>(self.max_function_defs)?;
        wr.write_u16::<BigEndian>(self.max_instruction_defs)?;
        wr.write_u16::<BigEndian>(self.max_stack_elements)?;
        wr.write_u16::<BigEndian>(self.max_size_of_instructions)?;
        wr.write_u16::<BigEndian>(self.max_component_elements)?;
        wr.write_u16::<BigEndian>(self.max_component_depth)?;
        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Owned(TrueTypeMaxpTable {
            // +1 since glyph 0 is always additionally added
            num_glyphs: u16::try_from(glyphs.len() + 1).ok().unwrap_or(u16::MAX),
            ..self.to_owned()
        })
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_maxp_table_true_type_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let maxp_table: MaxpTable = table
            .unpack_required_table("maxp", (), &mut cursor)
            .unwrap();

        match &maxp_table {
            MaxpTable::CFF(_) => panic!("Expected TrueType maxp table"),
            MaxpTable::TrueType(table) => {
                assert_eq!(table.num_glyphs, 8898);
                assert_eq!(table.max_points, 288);
                assert_eq!(table.max_contours, 41);
                assert_eq!(table.max_component_points, 152);
                assert_eq!(table.max_component_contours, 13);
                assert_eq!(table.max_zones, 2);
                assert_eq!(table.max_twilight_points, 144);
                assert_eq!(table.max_storage, 240);
                assert_eq!(table.max_function_defs, 141);
                assert_eq!(table.max_instruction_defs, 0);
                assert_eq!(table.max_stack_elements, 373);
                assert_eq!(table.max_size_of_instructions, 3596);
                assert_eq!(table.max_component_elements, 5);
                assert_eq!(table.max_component_depth, 4);
            }
        }

        // re-pack and compare
        let mut buffer = Vec::new();
        maxp_table.pack(&mut buffer).unwrap();
        assert_eq!(
            MaxpTable::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            maxp_table
        );
    }

    #[test]
    fn test_maxp_table_cff_encode_decode() {
        let data = vec![
            0x00, 0x00, 0x50, 0x00, // version
            0x22, 0xC2, // number glyphs
        ];
        let maxp_table = MaxpTable::unpack(&mut &data[..], ()).unwrap();

        match &maxp_table {
            MaxpTable::CFF(table) => {
                assert_eq!(table.num_glyphs, 8898);
            }
            MaxpTable::TrueType(_) => panic!("Expected CFF maxp table"),
        }

        // re-pack and compare
        let mut buffer = Vec::new();
        maxp_table.pack(&mut buffer).unwrap();
        assert_eq!(
            MaxpTable::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            maxp_table
        );
    }

    #[test]
    fn test_maxp_true_type_subset() {
        let maxp = TrueTypeMaxpTable {
            num_glyphs: 8898,
            max_points: 288,
            max_contours: 41,
            max_component_points: 152,
            max_component_contours: 13,
            max_zones: 2,
            max_twilight_points: 144,
            max_storage: 240,
            max_function_defs: 141,
            max_instruction_defs: 0,
            max_stack_elements: 373,
            max_size_of_instructions: 3596,
            max_component_elements: 5,
            max_component_depth: 4,
        };
        let glyphs = &[Glyph::new(1), Glyph::new(2), Glyph::new(3)];
        let subset = maxp.subset(glyphs, ());
        // 4 since glyph 0 is always included
        assert_eq!(subset.num_glyphs, 4);

        // everything else is unchangde
        assert_eq!(subset.max_points, maxp.max_points);
        assert_eq!(subset.max_contours, maxp.max_contours);
        assert_eq!(subset.max_component_points, maxp.max_component_points);
        assert_eq!(subset.max_component_contours, maxp.max_component_contours);
        assert_eq!(subset.max_zones, maxp.max_zones);
        assert_eq!(subset.max_twilight_points, maxp.max_twilight_points);
        assert_eq!(subset.max_storage, maxp.max_storage);
        assert_eq!(subset.max_function_defs, maxp.max_function_defs);
        assert_eq!(subset.max_instruction_defs, maxp.max_instruction_defs);
        assert_eq!(subset.max_stack_elements, maxp.max_stack_elements);
        assert_eq!(
            subset.max_size_of_instructions,
            maxp.max_size_of_instructions
        );
        assert_eq!(subset.max_component_elements, maxp.max_component_elements);
        assert_eq!(subset.max_component_depth, maxp.max_component_depth);

        // subset container struct
        let maxp_table = MaxpTable::TrueType(maxp.clone());
        assert_eq!(
            maxp_table.subset(glyphs, ()).into_owned(),
            MaxpTable::TrueType(subset.into_owned())
        );
    }

    #[test]
    fn test_maxp_cff_subset() {
        let maxp = MaxpTable::CFF(CffMaxpTable { num_glyphs: 10 });
        let subset = maxp.subset(&[Glyph::new(1), Glyph::new(2), Glyph::new(3)], ());
        // 4 since glyph 0 is always included
        assert_eq!(subset.num_glyphs(), 4)
    }
}
