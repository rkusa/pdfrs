use std::cell::RefCell;
use std::collections::HashMap;
use std::io;

use super::font::{Font, FontCollection, SingleFont, SubsetRef};
use crate::stream::{Stream, StreamRef};
use crate::writer::DocWriter;
use async_std::io::prelude::Write;
use async_std::io::prelude::WriteExt;
use otf::Glyph;
use serde::Serialize;
use serde_pdf::{Object, ObjectId, Reference};

pub struct OpenTypeFont {
    font: otf::OpenTypeFont,
    post_script_name: String,
    subsets: RefCell<Vec<UnicodeSubset>>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct UnicodeSubset {
    /// Mapping of UTF8 codepoints to codepoints in the subset.
    mapping: HashMap<char, u8>,
    mapping_inverted: HashMap<u8, char>,
    current_codepoint: u8,
}

impl OpenTypeFont {
    pub fn new(font: otf::OpenTypeFont) -> Self {
        OpenTypeFont {
            post_script_name: font
                .post_script_name()
                .unwrap_or_else(|| "Unknown".to_string()),
            font,
            subsets: RefCell::new(Vec::new()),
        }
    }

    pub fn from_slice(data: impl AsRef<[u8]>) -> Result<Self, io::Error> {
        Ok(OpenTypeFont::new(otf::OpenTypeFont::from_slice(data)?))
    }
}

impl Font for OpenTypeFont {
    fn base_name(&self) -> &str {
        &self.post_script_name
    }

    fn kerning(&self, _lhs: char, _rhs: char) -> Option<i32> {
        None
    }

    fn encode_into(&self, text: &str, buf: &mut Vec<u8>) -> Result<(SubsetRef, usize), io::Error> {
        let first = match text.chars().next() {
            None => return Ok((SubsetRef(0), 0)),
            Some(c) => c,
        };
        let mut subsets = self.subsets.borrow_mut();
        let ix = subsets
            .iter_mut()
            .enumerate()
            .find_map(|(i, s)| s.map_char(first).map(|_| i))
            .unwrap_or_else(|| {
                subsets.push(UnicodeSubset::new());
                subsets.len() - 1
            });
        let subset = &mut subsets[ix];

        let mut len = 0;
        buf.push(b'(');
        for (i, ch) in text.char_indices() {
            if ch < ' ' {
                continue;
            }
            if let Some(b) = subset.map_char(ch) {
                match b {
                    b'\\' => buf.extend(b"\\\\"),
                    b'(' => buf.extend(b"\\("),
                    b')' => buf.extend(b"\\)"),
                    b => buf.push(b),
                }
                len = i + ch.len_utf8();
            } else {
                break;
            }
        }
        buf.push(b')');

        Ok((SubsetRef(ix), len))
    }
}

#[async_trait::async_trait(?Send)]
impl FontCollection for OpenTypeFont {
    type FontRef = SingleFont;

    fn font(&self, _font: Self::FontRef) -> &dyn Font {
        self
    }

    async fn write_objects<W: Write + Unpin>(
        &self,
        _: Self::FontRef,
        subset_ref: SubsetRef,
        obj_id: ObjectId,
        doc: DocWriter<W>,
        compressed: bool,
    ) -> Result<DocWriter<W>, serde_pdf::Error> {
        let subsets = self.subsets.borrow();
        let subset = match subsets.get(subset_ref.font_id()) {
            Some(subset) => subset,
            None => return Ok(doc),
        };

        let mut glyphs = subset
            .chars()
            .filter_map(|pair| {
                pair.and_then(|(b, ch)| {
                    self.font
                        .glyph_id(ch as u32)
                        // remap char to new ascii character
                        .map(|index| (index, b as u32))
                })
            })
            .fold(HashMap::new(), |mut glyphs, (i, c)| {
                let glyph = glyphs.entry(i).or_insert_with(|| Glyph {
                    index: i,
                    code_points: Vec::with_capacity(1),
                });
                glyph.code_points.push(c);
                glyphs
            })
            .into_iter()
            .map(|(_, g)| g)
            .collect::<Vec<_>>();

        // sort for deterministic results
        glyphs.sort_by_key(|g| g.index);

        let new_font = self.font.subset_from_glyphs(&glyphs);

        let mut font_file = Stream::start(doc, true, true).await?;
        let font_file_ref = font_file.to_reference();
        new_font.to_async_writer(&mut font_file, true).await?;
        let doc = font_file.end().await?;

        let subset_name = format!("{}+{}", tag(subset_ref.0), self.post_script_name);

        let mut cmap = Stream::start(doc, compressed, false).await?;
        let cmap_ref = cmap.to_reference();
        write_cmap(&mut cmap, &subset_name, &subset).await?;
        let mut doc = cmap.end().await?;

        let mut flags = 0;
        if (new_font.is_fixed_pitch()) {
            flags |= 1 << 0;
        }
        if (new_font.is_serif()) {
            flags |= 1 << 1;
        }
        if (new_font.is_script()) {
            flags |= 1 << 3;
        }
        flags |= 1 << 5; // assume non-symbolic
        if (new_font.is_italic()) {
            flags |= 1 << 6;
        }

        let font_family = new_font.font_family_name();
        let font_obj = Object::new(
            obj_id.id(),
            obj_id.rev(),
            FontObject {
                subtype: FontType::TrueType,
                base_font: subset_name,
                first_char: subset.first_char(),
                last_char: subset.last_char(),
                widths: subset
                    .chars()
                    .map(|ch| ch.map(|(_, ch)| self.font.char_width(ch)).unwrap_or(0))
                    .collect(),
                font_descriptor: FontDescriptor {
                    font_name: &self.post_script_name,
                    font_family: font_family.as_deref(),
                    flags,
                    font_b_box: self.font.bbox(),
                    italic_angle: self.font.italic_angle(),
                    ascent: self.font.ascent(),
                    descent: self.font.descent(),
                    leading: self.font.line_gap(),
                    cap_height: self.font.cap_height(),
                    x_height: self.font.x_height(),
                    stem_v: 0, // unknown
                    font_file_2: font_file_ref,
                },
                encoding: FontEncoding::WinAnsiEncoding,
                to_unicode: cmap_ref,
            },
        );
        doc.write_object(font_obj).await?;
        Ok(doc)
    }
}

async fn write_cmap<W: Write + Unpin>(
    stream: &mut Stream<W>,
    subset_name: &str,
    subset: &UnicodeSubset,
) -> Result<(), serde_pdf::Error> {
    let cmap_name = serde_pdf::to_string(&subset_name)?;

    writeln!(stream, "/CIDInit /ProcSet findresource begin").await?;
    writeln!(stream, "12 dict begin").await?;
    writeln!(stream, "begincmap").await?;
    writeln!(stream, "/CIDSystemInfo 3 dict dup begin").await?;
    writeln!(stream, "  /Registry (Adobe) def").await?;
    writeln!(stream, "  /Ordering (Identity) def").await?;
    writeln!(stream, "  /Supplement 0 def").await?;
    writeln!(stream, "end def").await?;
    writeln!(stream, "/CMapName {}", cmap_name).await?;
    writeln!(stream, "/CMapType 2 def").await?;
    writeln!(stream, "1 begincodespacerange").await?;
    writeln!(stream, "<0000><ffff>").await?;
    writeln!(stream, "endcodespacerange").await?;
    writeln!(stream, "{} beginbfchar", subset.mapping_inverted.len()).await?;

    // TODO: try to use `bfrange` where possible?
    for pair in subset.chars() {
        if let Some((b, ch)) = pair {
            writeln!(stream, "<{:04x}> <{:04x}>", b, ch as u32).await?;
        }
    }

    writeln!(stream, "endbfchar").await?;
    writeln!(stream, "endcmap").await?;
    writeln!(stream, "CMapName currentdict /CMap defineresource pop").await?;
    writeln!(stream, "end").await?;
    writeln!(stream, "end").await?;

    Ok(())
}

#[derive(Serialize)]
enum FontType {
    TrueType,
}

#[derive(Serialize)]
enum FontEncoding {
    WinAnsiEncoding,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "Font")]
struct FontObject<'a> {
    subtype: FontType,
    base_font: String,
    first_char: u8,
    last_char: u8,
    widths: Vec<u16>,
    font_descriptor: FontDescriptor<'a>,
    encoding: FontEncoding,
    to_unicode: Reference<StreamRef>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct FontDescriptor<'a> {
    font_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_family: Option<&'a str>,
    flags: u32,
    font_b_box: [i16; 4],
    italic_angle: i32,
    ascent: i16,
    descent: i16,
    leading: i16,
    cap_height: i16,
    x_height: i16,
    stem_v: u16,
    font_file_2: Reference<StreamRef>,
}

impl UnicodeSubset {
    fn new() -> Self {
        UnicodeSubset {
            mapping: HashMap::new(),
            mapping_inverted: HashMap::new(),
            current_codepoint: 32, // 32 to start with first character after space
        }
    }

    fn map_char(&mut self, ch: char) -> Option<u8> {
        if let Some(b) = self.mapping.get(&ch) {
            return Some(*b);
        }

        if self.current_codepoint == u8::MAX {
            return None;
        }

        self.current_codepoint += 1;
        self.mapping.insert(ch, self.current_codepoint);
        self.mapping_inverted.insert(self.current_codepoint, ch);
        Some(self.current_codepoint)
    }

    fn chars<'a>(&'a self) -> impl Iterator<Item = Option<(u8, char)>> + 'a {
        (self.first_char()..self.last_char())
            .map(move |b| self.mapping_inverted.get(&b).map(|ch| (b, *ch)))
    }

    fn first_char(&self) -> u8 {
        33
    }

    fn last_char(&self) -> u8 {
        self.current_codepoint + 1
    }
}

fn tag(n: usize) -> String {
    let tag = format!("{:06}", n);
    tag.as_bytes().iter().map(|b| (b + 17) as char).collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_subsets() {
        let mut subset = UnicodeSubset::new();
        assert_eq!(subset.map_char(' '), Some(33));
        assert_eq!(subset.map_char('a'), Some(34));
        assert_eq!(subset.map_char('\u{94}'), Some(35));
        assert_eq!(subset.map_char('░'), Some(36));
    }

    #[test]
    fn test_tag() {
        assert_eq!(tag(0), "AAAAAA");
        assert_eq!(tag(123456), "BCDEFG");
    }
}
