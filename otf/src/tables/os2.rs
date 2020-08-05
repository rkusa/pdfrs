use std::io;

use super::FontTable;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table consists of a set of metrics and other data that are required for a font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/os2
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6OS2.html
#[derive(Debug, PartialEq)]
pub struct Os2Table {
    version: u16,
    x_avg_char_width: i16,
    us_weight_class: u16,
    us_width_class: u16,
    fs_type: u16,
    y_subscript_x_size: i16,
    y_subscript_y_size: i16,
    y_subscript_x_offset: i16,
    y_subscript_y_offset: i16,
    y_superscript_x_size: i16,
    y_superscript_y_size: i16,
    y_superscript_x_offset: i16,
    y_superscript_y_offset: i16,
    y_strikeout_size: i16,
    y_strikeout_position: i16,
    s_family_class: i16,
    panose: [u8; 10],
    ul_unicode_range1: u32,
    ul_unicode_range2: u32,
    ul_unicode_range3: u32,
    ul_unicode_range4: u32,
    ach_vend_id: [u8; 4],
    fs_selection: u16,
    us_first_char_index: u16,
    us_last_char_index: u16,
    s_typo_ascender: i16,
    s_typo_descender: i16,
    s_typo_line_gap: i16,
    us_win_ascent: u16,
    us_win_descent: u16,
    // the following fields are only available for version > 0
    ul_code_page_range1: u32,
    ul_code_page_range2: u32,
    // the following fields are only available for version > 1
    sx_height: i16,
    s_cap_height: i16,
    us_default_char: u16,
    us_break_char: u16,
    us_max_context: u16,
    // the following fields are only available for version > 4
    us_lower_optical_point_size: u16,
    us_upper_optical_point_size: u16,
}

impl<'a> FontTable<'a> for Os2Table {
    type Dep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::Dep) -> Result<Self, io::Error> {
        let version = rd.read_u16::<BigEndian>()?;
        let x_avg_char_width = rd.read_i16::<BigEndian>()?;
        let us_weight_class = rd.read_u16::<BigEndian>()?;
        let us_width_class = rd.read_u16::<BigEndian>()?;
        let fs_type = rd.read_u16::<BigEndian>()?;
        let y_subscript_x_size = rd.read_i16::<BigEndian>()?;
        let y_subscript_y_size = rd.read_i16::<BigEndian>()?;
        let y_subscript_x_offset = rd.read_i16::<BigEndian>()?;
        let y_subscript_y_offset = rd.read_i16::<BigEndian>()?;
        let y_superscript_x_size = rd.read_i16::<BigEndian>()?;
        let y_superscript_y_size = rd.read_i16::<BigEndian>()?;
        let y_superscript_x_offset = rd.read_i16::<BigEndian>()?;
        let y_superscript_y_offset = rd.read_i16::<BigEndian>()?;
        let y_strikeout_size = rd.read_i16::<BigEndian>()?;
        let y_strikeout_position = rd.read_i16::<BigEndian>()?;
        let s_family_class = rd.read_i16::<BigEndian>()?;
        let mut panose = [0; 10];
        rd.read_exact(&mut panose)?;
        let ul_unicode_range1 = rd.read_u32::<BigEndian>()?;
        let ul_unicode_range2 = rd.read_u32::<BigEndian>()?;
        let ul_unicode_range3 = rd.read_u32::<BigEndian>()?;
        let ul_unicode_range4 = rd.read_u32::<BigEndian>()?;
        let mut ach_vend_id = [0; 4];
        rd.read_exact(&mut ach_vend_id)?;
        let fs_selection = rd.read_u16::<BigEndian>()?;
        let us_first_char_index = rd.read_u16::<BigEndian>()?;
        let us_last_char_index = rd.read_u16::<BigEndian>()?;
        let s_typo_ascender = rd.read_i16::<BigEndian>()?;
        let s_typo_descender = rd.read_i16::<BigEndian>()?;
        let s_typo_line_gap = rd.read_i16::<BigEndian>()?;
        let us_win_ascent = rd.read_u16::<BigEndian>()?;
        let us_win_descent = rd.read_u16::<BigEndian>()?;

        let ul_code_page_range1 = if version > 0 {
            rd.read_u32::<BigEndian>()?
        } else {
            0
        };
        let ul_code_page_range2 = if version > 0 {
            rd.read_u32::<BigEndian>()?
        } else {
            0
        };

        let sx_height = if version > 1 {
            rd.read_i16::<BigEndian>()?
        } else {
            0
        };
        let s_cap_height = if version > 1 {
            rd.read_i16::<BigEndian>()?
        } else {
            0
        };
        let us_default_char = if version > 1 {
            rd.read_u16::<BigEndian>()?
        } else {
            0
        };
        let us_break_char = if version > 1 {
            rd.read_u16::<BigEndian>()?
        } else {
            0
        };
        let us_max_context = if version > 1 {
            rd.read_u16::<BigEndian>()?
        } else {
            0
        };

        let us_lower_optical_point_size = if version > 4 {
            rd.read_u16::<BigEndian>()?
        } else {
            0
        };
        let us_upper_optical_point_size = if version > 4 {
            rd.read_u16::<BigEndian>()?
        } else {
            0
        };

        Ok(Os2Table {
            version,
            x_avg_char_width,
            us_weight_class,
            us_width_class,
            fs_type,
            y_subscript_x_size,
            y_subscript_y_size,
            y_subscript_x_offset,
            y_subscript_y_offset,
            y_superscript_x_size,
            y_superscript_y_size,
            y_superscript_x_offset,
            y_superscript_y_offset,
            y_strikeout_size,
            y_strikeout_position,
            s_family_class,
            panose,
            ul_unicode_range1,
            ul_unicode_range2,
            ul_unicode_range3,
            ul_unicode_range4,
            ach_vend_id,
            fs_selection,
            us_first_char_index,
            us_last_char_index,
            s_typo_ascender,
            s_typo_descender,
            s_typo_line_gap,
            us_win_ascent,
            us_win_descent,
            ul_code_page_range1,
            ul_code_page_range2,
            sx_height,
            s_cap_height,
            us_default_char,
            us_break_char,
            us_max_context,
            us_lower_optical_point_size,
            us_upper_optical_point_size,
        })
    }

    fn pack<W: io::Write>(&'a self, wr: &mut W, _: Self::Dep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.version)?;
        wr.write_i16::<BigEndian>(self.x_avg_char_width)?;
        wr.write_u16::<BigEndian>(self.us_weight_class)?;
        wr.write_u16::<BigEndian>(self.us_width_class)?;
        wr.write_u16::<BigEndian>(self.fs_type)?;
        wr.write_i16::<BigEndian>(self.y_subscript_x_size)?;
        wr.write_i16::<BigEndian>(self.y_subscript_y_size)?;
        wr.write_i16::<BigEndian>(self.y_subscript_x_offset)?;
        wr.write_i16::<BigEndian>(self.y_subscript_y_offset)?;
        wr.write_i16::<BigEndian>(self.y_superscript_x_size)?;
        wr.write_i16::<BigEndian>(self.y_superscript_y_size)?;
        wr.write_i16::<BigEndian>(self.y_superscript_x_offset)?;
        wr.write_i16::<BigEndian>(self.y_superscript_y_offset)?;
        wr.write_i16::<BigEndian>(self.y_strikeout_size)?;
        wr.write_i16::<BigEndian>(self.y_strikeout_position)?;
        wr.write_i16::<BigEndian>(self.s_family_class)?;
        wr.write_all(&self.panose)?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range1)?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range2)?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range3)?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range4)?;
        wr.write_all(&self.ach_vend_id)?;
        wr.write_u16::<BigEndian>(self.fs_selection)?;
        wr.write_u16::<BigEndian>(self.us_first_char_index)?;
        wr.write_u16::<BigEndian>(self.us_last_char_index)?;
        wr.write_i16::<BigEndian>(self.s_typo_ascender)?;
        wr.write_i16::<BigEndian>(self.s_typo_descender)?;
        wr.write_i16::<BigEndian>(self.s_typo_line_gap)?;
        wr.write_u16::<BigEndian>(self.us_win_ascent)?;
        wr.write_u16::<BigEndian>(self.us_win_descent)?;

        if self.version > 0 {
            wr.write_u32::<BigEndian>(self.ul_code_page_range1)?;
            wr.write_u32::<BigEndian>(self.ul_code_page_range2)?;
        }

        if self.version > 1 {
            wr.write_i16::<BigEndian>(self.sx_height)?;
            wr.write_i16::<BigEndian>(self.s_cap_height)?;
            wr.write_u16::<BigEndian>(self.us_default_char)?;
            wr.write_u16::<BigEndian>(self.us_break_char)?;
            wr.write_u16::<BigEndian>(self.us_max_context)?;
        }

        if self.version > 4 {
            wr.write_u16::<BigEndian>(self.us_lower_optical_point_size)?;
            wr.write_u16::<BigEndian>(self.us_upper_optical_point_size)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_os2_table_encode_decode() {
        let data = include_bytes!("../../tests/fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let os2_table: Os2Table = table
            .unpack_required_table("OS/2", (), &mut cursor)
            .unwrap();

        dbg!(&os2_table);

        assert_eq!(
            os2_table,
            Os2Table {
                version: 4,
                x_avg_char_width: 500,
                us_weight_class: 400,
                us_width_class: 5,
                fs_type: 0,
                y_subscript_x_size: 665,
                y_subscript_y_size: 716,
                y_subscript_x_offset: 0,
                y_subscript_y_offset: 143,
                y_superscript_x_size: 0,
                y_superscript_y_size: 0,
                y_superscript_x_offset: 0,
                y_superscript_y_offset: 0,
                y_strikeout_size: 51,
                y_strikeout_position: 265,
                s_family_class: 2057,
                panose: [2, 0, 5, 9, 0, 0, 0, 0, 0, 0],
                ul_unicode_range1: 3758097151,
                ul_unicode_range2: 1379991039,
                ul_unicode_range3: 262144,
                ul_unicode_range4: 0,
                ach_vend_id: *b"BE5N",
                fs_selection: 192,
                us_first_char_index: 13,
                us_last_char_index: 65535,
                s_typo_ascender: 977,
                s_typo_descender: -272,
                s_typo_line_gap: 0,
                us_win_ascent: 977,
                us_win_descent: 272,
                ul_code_page_range1: 536871199,
                ul_code_page_range2: 3288334336,
                sx_height: 530,
                s_cap_height: 735,
                us_default_char: 0,
                us_break_char: 32,
                us_max_context: 8,
                us_lower_optical_point_size: 0,
                us_upper_optical_point_size: 0,
            }
        );

        // re-pack and compare
        let mut buffer = Vec::new();
        os2_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            Os2Table::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            os2_table
        );
    }
}
