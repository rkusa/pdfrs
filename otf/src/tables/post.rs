use std::io::{self, Cursor, Read};

use super::{FontData, FontTable};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table contains additional information needed to use OTF fonts on PostScript printers.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/post
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6post.html
#[derive(Debug, PartialEq, Clone)]
pub struct PostTable {
    pub(crate) major_version: u16,
    pub(crate) minor_version: u16,
    /// Italic angle in counter-clockwise degrees from the vertical. Zero for upright text, negative
    /// for text that leans to the right (forward).
    // TODO: type fixed
    pub(crate) italic_angle: i32,
    /// This is the suggested distance of the top of the underline from the baseline (negative
    /// values indicate below baseline).
    pub(crate) underline_position: i16,
    /// Suggested values for the underline thickness.
    pub(crate) underline_thickness: i16,
    /// Set to 0 if the font is proportionally spaced, non-zero if the font is not proportionally
    /// spaced (i.e. monospaced).
    pub(crate) is_fixed_path: u32,
    /// Minimum memory usage when an OpenType font is downloaded.
    pub(crate) min_mem_type42: u32,
    /// Maximum memory usage when an OpenType font is downloaded.
    pub(crate) max_mem_type42: u32,
    /// Minimum memory usage when an OpenType font is downloaded as a Type 1 font.
    pub(crate) min_mem_type1: u32,
    /// Maximum memory usage when an OpenType font is downloaded as a Type 1 font.
    pub(crate) max_mem_type1: u32,

    /// Additional content for format 2 and format 4 post tables.
    // TODO?: Actually parse the content?
    pub(crate) addition: Vec<u8>,
}

impl<'a> FontTable<'a, (), (), ()> for PostTable {
    fn name() -> &'static str {
        "post"
    }
}

impl<'a> FontData<'a> for PostTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let major_version = rd.read_u16::<BigEndian>()?;
        let minor_version = rd.read_u16::<BigEndian>()?;
        let italic_angle = rd.read_i32::<BigEndian>()?;
        let underline_position = rd.read_i16::<BigEndian>()?;
        let underline_thickness = rd.read_i16::<BigEndian>()?;
        let is_fixed_path = rd.read_u32::<BigEndian>()?;
        let min_mem_type42 = rd.read_u32::<BigEndian>()?;
        let max_mem_type42 = rd.read_u32::<BigEndian>()?;
        let min_mem_type1 = rd.read_u32::<BigEndian>()?;
        let max_mem_type1 = rd.read_u32::<BigEndian>()?;
        let mut addition = Vec::new(); // TODO: guess capacity?
        rd.read_to_end(&mut addition)?;

        Ok(PostTable {
            major_version,
            minor_version,
            italic_angle,
            underline_position,
            underline_thickness,
            is_fixed_path,
            min_mem_type42,
            max_mem_type42,
            min_mem_type1,
            max_mem_type1,
            addition,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.major_version)?;
        wr.write_u16::<BigEndian>(self.minor_version)?;
        wr.write_i32::<BigEndian>(self.italic_angle)?;
        wr.write_i16::<BigEndian>(self.underline_position)?;
        wr.write_i16::<BigEndian>(self.underline_thickness)?;
        wr.write_u32::<BigEndian>(self.is_fixed_path)?;
        wr.write_u32::<BigEndian>(self.min_mem_type42)?;
        wr.write_u32::<BigEndian>(self.max_mem_type42)?;
        wr.write_u32::<BigEndian>(self.min_mem_type1)?;
        wr.write_u32::<BigEndian>(self.max_mem_type1)?;
        wr.write_all(&self.addition)?;
        Ok(())
    }

    // TODO: implement subset and update mem usage and addition based on version
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_post_table_encode_decode() {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let post_table: PostTable = table.unpack_required_table((), &mut cursor).unwrap();

        assert_eq!(post_table.major_version, 3);
        assert_eq!(post_table.minor_version, 0);
        assert_eq!(post_table.italic_angle, 0);
        assert_eq!(post_table.underline_position, -50);
        assert_eq!(post_table.underline_thickness, 50);
        assert_eq!(post_table.is_fixed_path, 1);
        assert_eq!(post_table.min_mem_type42, 0);
        assert_eq!(post_table.max_mem_type42, 8898);
        assert_eq!(post_table.min_mem_type1, 0);
        assert_eq!(post_table.max_mem_type1, 1);

        // re-pack and compare
        let mut buffer = Vec::new();
        post_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            PostTable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            post_table
        );
    }
}
