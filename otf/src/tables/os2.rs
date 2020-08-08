use std::io;

use super::FontTable;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// This table consists of a set of metrics and other data that are required for a font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/os2
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6OS2.html
#[derive(Debug, PartialEq, Clone)]
pub struct Os2Table {
    /// The version number for the OS/2 table: 0x0000 to 0x0005.
    version: u16,
    /// he Average Character Width parameter specifies the arithmetic average of the escapement
    /// (width) of all non-zero width glyphs in the font.
    x_avg_char_width: i16,
    /// Indicates the visual weight (degree of blackness or thickness of strokes) of the characters
    /// in the font. Values from 1 to 1000 are valid.
    us_weight_class: u16,
    /// Indicates a relative change from the normal aspect ratio (width to height ratio) as
    /// specified by a font designer for the glyphs in a font.
    us_width_class: u16,
    /// Indicates font embedding licensing rights for the font.
    // TODO: use to prevent embedding certain fonts into a PDF?
    fs_type: u16,
    /// The recommended horizontal size in font design units for subscripts for this font.
    y_subscript_x_size: i16,
    /// The recommended vertical size in font design units for subscripts for this font.
    y_subscript_y_size: i16,
    /// The recommended horizontal offset in font design units for subscripts for this font.
    y_subscript_x_offset: i16,
    /// The recommended vertical offset in font design units from the baseline for subscripts for
    /// this font.
    y_subscript_y_offset: i16,
    /// The recommended horizontal size in font design units for superscripts for this font.
    y_superscript_x_size: i16,
    /// The recommended vertical size in font design units for superscripts for this font.
    y_superscript_y_size: i16,
    /// The recommended horizontal offset in font design units for superscripts for this font.
    y_superscript_x_offset: i16,
    /// The recommended vertical offset in font design units from the baseline for superscripts for
    /// this font.
    y_superscript_y_offset: i16,
    /// Thickness of the strikeout stroke in font design units.
    y_strikeout_size: i16,
    /// The position of the top of the strikeout stroke relative to the baseline in font design units.
    y_strikeout_position: i16,
    /// This parameter is a classification of font-family design.
    s_family_class: i16,
    /// This 10-byte series of numbers is used to describe the visual characteristics of a given
    /// typeface.
    panose: [u8; 10],
    /// This field is used to specify the Unicode blocks or ranges encompassed by the font file in
    /// 'cmap' subtables for platform 3, encoding ID 1 (Microsoft platform, Unicode BMP) and
    /// platform 3, encoding ID 10 (Microsoft platform, Unicode full repertoire). If a bit is set
    /// (1), then the Unicode ranges assigned to that bit are considered functional. If the bit is
    /// clear (0), then the range is not considered functional.
    ul_unicode_range: [u32; 4],
    /// The four-character identifier for the vendor of the given type face.
    ach_vend_id: [u8; 4],
    /// Contains information concerning the nature of the font patterns.
    fs_selection: u16,
    /// The minimum Unicode index (character code) in this font, according to the 'cmap' subtable
    /// for platform ID 3 and platform- specific encoding ID 0 or 1. For most fonts supporting
    /// Win-ANSI or other character sets, this value would be 0x0020. This field cannot represent
    /// supplementary character values (codepoints greater than 0xFFFF). Fonts that support
    /// supplementary characters should set the value in this field to 0xFFFF if the minimum index
    /// value is a supplementary character.
    us_first_char_index: u16,
    /// The maximum Unicode index (character code) in this font, according to the 'cmap' subtable
    /// for platform ID 3 and encoding ID 0 or 1. This value depends on which character sets the
    /// font supports. This field cannot represent supplementary character values (codepoints
    /// greater than 0xFFFF). Fonts that support supplementary characters should set the value in
    /// this field to 0xFFFF.
    us_last_char_index: u16,
    /// The typographic ascender for this font. This field should be combined with the
    /// `s_typo_descender` and `s_typo_line_gap` values to determine default line spacing.
    s_typo_ascender: i16,
    /// The typographic descender for this font. This field should be combined with the
    /// `s_typo_ascender` and `s_typo_line_gap` values to determine default line spacing.
    s_typo_descender: i16,
    /// The typographic line gap for this font. This field should be combined with the
    /// `s_typo_ascender` and `s_typo_descender` values to determine default line spacing.
    s_typo_line_gap: i16,
    /// The “Windows ascender” metric. This should be used to specify the height above the baseline
    /// for a clipping region.
    us_win_ascent: u16,
    /// The “Windows descender” metric. This should be used to specify the vertical extent below the
    /// baseline for a clipping region.
    us_win_descent: u16,

    // the following fields are only available for version > 0
    /// This field is used to specify the code pages encompassed by the font file in the 'cmap'
    /// subtable for platform 3, encoding ID 1 (Microsoft platform, Unicode BMP).
    ul_code_page_range: [u32; 2],

    // the following fields are only available for version > 1
    /// This metric specifies the distance between the baseline and the approximate height of
    /// non-ascending lowercase letters measured in FUnits.
    sx_height: i16,
    /// This metric specifies the distance between the baseline and the approximate height of
    /// uppercase letters measured in FUnits.
    s_cap_height: i16,
    /// This is the Unicode code point, in UTF-16 encoding, of a character that can be used for a
    /// default glyph if a requested character is not supported in the font. If the value of this
    /// field is zero, glyph ID 0 is to be used for the default character.
    us_default_char: u16,
    /// This is the Unicode code point, in UTF-16 encoding, of a character that can be used as a
    /// default break character. The break character is used to separate words and justify text.
    /// Most fonts specify U+0020 SPACE as the break character.
    us_break_char: u16,
    /// The maximum length of a target glyph context for any feature in this font.
    us_max_context: u16,

    // the following fields are only available for version > 4
    /// This value is the lower value of the size range for which this font has been designed.
    us_lower_optical_point_size: u16,
    /// This value is the upper value of the size range for which this font has been designed.
    us_upper_optical_point_size: u16,
}

impl<'a> FontTable<'a> for Os2Table {
    type UnpackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read>(rd: &mut R, _: Self::UnpackDep) -> Result<Self, io::Error> {
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
        let ul_unicode_range = [
            rd.read_u32::<BigEndian>()?,
            rd.read_u32::<BigEndian>()?,
            rd.read_u32::<BigEndian>()?,
            rd.read_u32::<BigEndian>()?,
        ];
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

        let ul_code_page_range = if version > 0 {
            [rd.read_u32::<BigEndian>()?, rd.read_u32::<BigEndian>()?]
        } else {
            [0, 0]
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
            ul_unicode_range,
            ach_vend_id,
            fs_selection,
            us_first_char_index,
            us_last_char_index,
            s_typo_ascender,
            s_typo_descender,
            s_typo_line_gap,
            us_win_ascent,
            us_win_descent,
            ul_code_page_range,
            sx_height,
            s_cap_height,
            us_default_char,
            us_break_char,
            us_max_context,
            us_lower_optical_point_size,
            us_upper_optical_point_size,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W) -> Result<(), io::Error> {
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
        wr.write_u32::<BigEndian>(self.ul_unicode_range[0])?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range[1])?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range[2])?;
        wr.write_u32::<BigEndian>(self.ul_unicode_range[3])?;
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
            wr.write_u32::<BigEndian>(self.ul_code_page_range[0])?;
            wr.write_u32::<BigEndian>(self.ul_code_page_range[1])?;
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

    // TODO: implement subsetting to update the following values?
    // - x_avg_char_width
    // - us_first_char_index
    // - us_default_char
    // - us_break_char
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
                ul_unicode_range: [3758097151, 1379991039, 262144, 0],
                ach_vend_id: *b"BE5N",
                fs_selection: 192,
                us_first_char_index: 13,
                us_last_char_index: 65535,
                s_typo_ascender: 977,
                s_typo_descender: -272,
                s_typo_line_gap: 0,
                us_win_ascent: 977,
                us_win_descent: 272,
                ul_code_page_range: [536871199, 3288334336],
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
        os2_table.pack(&mut buffer).unwrap();
        assert_eq!(
            Os2Table::unpack(&mut Cursor::new(buffer), ()).unwrap(),
            os2_table
        );
    }
}
