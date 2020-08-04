use std::io;

use crate::packed::Packed;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table establishes the memory requirements for this font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/maxp
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6maxp.html
#[derive(Debug, PartialEq)]
pub enum MaxpTable {
    // Version 0.5
    CFF(CffMaxpTable),
    // Version 1.0
    TrueType(TrueTypeMaxpTable),
}

#[derive(Debug, PartialEq)]
pub struct CffMaxpTable {
    /// The number of glyphs in the font.
    num_glyphs: u16,
}

#[derive(Debug, PartialEq)]
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

impl<'a> Packed<'a> for MaxpTable {
    type Dep = ();

    fn unpack<R: io::Read>(mut rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
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

    fn pack<W: io::Write>(&'a self, mut wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        match self {
            MaxpTable::CFF(table) => {
                // version
                wr.write_u32::<BigEndian>(0x00005000)?;
                table.pack(&mut wr, ())?;
            }
            MaxpTable::TrueType(table) => {
                // version
                wr.write_u32::<BigEndian>(0x00010000)?;
                table.pack(&mut wr, ())?;
            }
        }

        Ok(())
    }
}

impl<'a> Packed<'a> for CffMaxpTable {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        Ok(CffMaxpTable {
            num_glyphs: rd.read_u16::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.num_glyphs)?;
        Ok(())
    }
}

impl<'a> Packed<'a> for TrueTypeMaxpTable {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
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

    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
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
        maxp_table.pack(&mut buffer, ()).unwrap();
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
        maxp_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            MaxpTable::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            maxp_table
        );
    }
}
