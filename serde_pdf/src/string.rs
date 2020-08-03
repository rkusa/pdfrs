use crate::ser::NAME_RAW;
use serde::{Serialize, Serializer};
use std::borrow::Cow;

pub enum PdfString {
    Hex(String),
    Literal(String),
}

pub enum PdfStr<'a> {
    Hex(&'a str),
    Literal(&'a str),
}

impl Serialize for PdfString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self {
            PdfString::Hex(ref s) => to_hex(s),
            PdfString::Literal(ref s) => to_literal(s),
        };
        serializer.serialize_newtype_struct(NAME_RAW, &s)
    }
}

impl<'a> Serialize for PdfStr<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self {
            PdfStr::Hex(s) => to_hex(s),
            PdfStr::Literal(s) => to_literal(s),
        };
        serializer.serialize_newtype_struct(NAME_RAW, &s)
    }
}

fn to_hex(s: &str) -> String {
    fn hex_from_digit(num: u8) -> char {
        if num < 10 {
            (b'0' + num) as char
        } else {
            (b'A' + num - 10) as char
        }
    }

    let mut buf = String::with_capacity(s.len() * 2);
    for ch in s.as_bytes() {
        let ch = ch - 31;
        buf.push(hex_from_digit(ch / 16));
        buf.push(hex_from_digit(ch % 16))
    }
    format!("<{}>", buf)
}

fn to_literal(s: &str) -> String {
    let mut buf = s.to_string();

    fn unicode_to_win1252(ch: char) -> Option<u32> {
        match ch {
            '\u{20ac}' => Some(128),
            '\u{201a}' => Some(130),
            '\u{0192}' => Some(131),
            '\u{201e}' => Some(132),
            '\u{2026}' => Some(133),
            '\u{2020}' => Some(134),
            '\u{2021}' => Some(135),
            '\u{02c6}' => Some(136),
            '\u{2030}' => Some(137),
            '\u{0160}' => Some(138),
            '\u{2039}' => Some(139),
            '\u{0152}' => Some(140),
            '\u{017d}' => Some(142),
            '\u{2018}' => Some(145),
            '\u{2019}' => Some(146),
            '\u{201c}' => Some(147),
            '\u{201d}' => Some(148),
            '\u{2022}' => Some(149),
            '\u{2013}' => Some(150),
            '\u{2014}' => Some(151),
            '\u{02dc}' => Some(152),
            '\u{2122}' => Some(153),
            '\u{0161}' => Some(154),
            '\u{203a}' => Some(155),
            '\u{0153}' => Some(156),
            '\u{017e}' => Some(158),
            '\u{0178}' => Some(159),
            _ => ch.to_digit(10),
        }
    }

    let mut i = 0;

    let replacements: Vec<(usize, usize, Cow<str>)> =
        buf.char_indices()
            .filter_map(|(i, ch)| match ch {
                '\\' => Some((i, 1, Cow::Borrowed(r#"\\"#))),
                '(' => Some((i, 1, Cow::Borrowed(r#"\("#))),
                ')' => Some((i, 1, Cow::Borrowed(r#"\)"#))),
                _ if !ch.is_ascii() => unicode_to_win1252(ch)
                    .map(|d| (i, ch.len_utf8(), Cow::Owned(format!("\\{:o}", d)))),
                _ => None,
            })
            .collect();

    for (i, l, rep) in replacements.iter().rev() {
        buf.replace_range(i..&(i + l), rep);
    }

    format!("({})", buf)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ser::to_string;

    #[test]
    fn test_serialize_hex_string() {
        let s = PdfString::Hex(String::from("foobar"));
        assert_eq!(to_string(&s).unwrap(), "<475050434253>");
    }

    #[test]
    fn test_serialize_hex_str() {
        let s = PdfStr::Hex("foobar");
        assert_eq!(to_string(&s).unwrap(), "<475050434253>");
    }

    #[test]
    fn test_serialize_literal_string() {
        let s = PdfString::Literal(String::from(r#"0ab(\fo)?!€"#));
        assert_eq!(to_string(&s).unwrap(), r#"(0ab\(\\fo\)?!\200)"#);
    }

    #[test]
    fn test_serialize_literal_str() {
        let s = PdfStr::Literal(r#"0ab(\fo)?!€"#);
        assert_eq!(to_string(&s).unwrap(), r#"(0ab\(\\fo\)?!\200)"#);
    }
}
