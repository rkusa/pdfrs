use std::io;

use super::FontTable;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table contains additional information needed to use OTF fonts on PostScript printers.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/post
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6post.html
#[derive(Debug, PartialEq)]
pub struct PostTable {
    major_version: u16,
    minor_version: u16,
    /// Italic angle in counter-clockwise degrees from the vertical. Zero for upright text, negative
    /// for text that leans to the right (forward).
    // TODO: type fixed
    italic_angle: i32,
    /// This is the suggested distance of the top of the underline from the baseline (negative
    /// values indicate below baseline).
    underline_position: i16,
    /// Suggested values for the underline thickness.
    underline_thickness: i16,
    /// Set to 0 if the font is proportionally spaced, non-zero if the font is not proportionally
    /// spaced (i.e. monospaced).
    is_fixed_path: u32,
    /// Minimum memory usage when an OpenType font is downloaded.
    min_mem_type42: u32,
    /// Maximum memory usage when an OpenType font is downloaded.
    max_mem_type42: u32,
    /// Minimum memory usage when an OpenType font is downloaded as a Type 1 font.
    min_mem_type1: u32,
    /// Maximum memory usage when an OpenType font is downloaded as a Type 1 font.
    max_mem_type1: u32,

    /// Additional content for format 2 and format 4 post tables.
    // TODO?: Actually parse the content?
    addition: Vec<u8>,
}

impl<'a> FontTable<'a> for PostTable {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
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

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        // TODO: update addition based on actual version and font content
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
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_post_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let post_table: PostTable = table
            .unpack_required_table("post", (), &mut cursor)
            .unwrap();

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
            PostTable::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            post_table
        );
    }
}
