use std::collections::HashMap;
use std::io::{self, Write};
use std::mem;

use crate::page::{Page, Pages, Resources};
use crate::stream::{Stream, StreamRef};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_pdf::{to_writer, Object, ObjectId, PdfStr, PdfString, Reference};

#[cfg(test)]
use chrono::TimeZone;
#[cfg(not(test))]
use uuid::Uuid;

const RESERVED_PAGES_ID: usize = 1;
// const PAGES_ID: ObjectId = ObjectId::new(RESERVED_PAGES_ID, 0);
// const PAGES_REFERENCE: Reference<Pages<'_>> = Reference::new(PAGES_ID);

pub struct IdSeq {
    next_id: usize,
}

pub struct Document<'a, W: io::Write> {
    out: Writer<W>,
    id_seq: IdSeq,
    pages: Vec<Reference<Page<'a>>>,
}

enum Writer<W: io::Write> {
    Doc(DocWriter<W>),
    Stream(Stream<W>),
    Null,
}

impl<'a, W> Document<'a, W>
where
    W: io::Write,
{
    pub fn new(out: W) -> Result<Self, io::Error> {
        let mut out = DocWriter::new(out);

        // The PDF format mandates that we add at least 4 commented binary characters
        // (ASCII value >= 128), so that generic tools have a chance to detect
        // that it's a binary file
        write!(out, "%PDF-1.6\n%")?;
        out.write_all(&[255, 255, 255, 255, '\n' as u8, '\n' as u8])?;

        Ok(Document {
            out: Writer::Doc(out),
            id_seq: IdSeq {
                next_id: RESERVED_PAGES_ID + 1,
            },
            pages: Vec::new(),
        })
    }

    fn new_object<D: Serialize>(&mut self, value: D) -> Object<D> {
        Object::new(self.id_seq.next(), 0, value)
    }

    fn new_stream(&mut self) {
        let out = mem::replace(&mut self.out, Writer::Null);
        self.out = out.into_stream(&mut self.id_seq);
    }

    fn write_object<D: Serialize>(&mut self, value: D) -> Result<Reference<D>, serde_pdf::Error> {
        let obj = self.new_object(value);
        let r = obj.to_reference();
        self.write(obj)?;
        return Ok(r);
    }

    fn write<D: Serialize>(&mut self, obj: Object<D>) -> Result<(), serde_pdf::Error> {
        match self.out {
            Writer::Stream(_) => {
                // TODO: maybe close stream instead of panicing
                unreachable!();
            }
            Writer::Doc(ref mut w) => {
                w.xref.insert(obj.id(), w.len);
                w.add_xref(obj.id());
                serde_pdf::to_writer(w, &obj)
            }
            Writer::Null => {
                unreachable!();
            }
        }
    }

    fn start_page(&mut self) {
        self.new_stream();
    }

    fn end_page(&mut self) -> Result<(), serde_pdf::Error> {
        if let Some(content_ref) = self.out.end_stream()? {
            // TODO: move to consts once const_fn landed
            let id = ObjectId::new(RESERVED_PAGES_ID, 0);
            let reference: Reference<Pages<'_>> = Reference::new(id);

            let page = Page {
                parent: reference,
                resources: Resources {
                    // while obsolete since PDF 1.4, still here for compatibility reasons, and simply
                    // setting all possible values ...
                    proc_set: vec!["PDF", "Text", "ImageB", "ImageC", "ImageI"],
                },
                contents: vec![content_ref],
            };

            let page_ref = self.write_object(page)?;
            self.pages.push(page_ref);
        }

        Ok(())
    }

    pub fn end(mut self) -> Result<(), serde_pdf::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Catalog<'a> {
            pages: Reference<Pages<'a>>,
        }

        self.start_page();
        self.end_page()?;

        let kids = mem::replace(&mut self.pages, Vec::new());
        let pages = Object::new(
            RESERVED_PAGES_ID,
            0,
            Pages {
                media_box: (0.0, 0.0, 595.296, 841.896),
                count: kids.len(),
                kids,
            },
        );
        let pages_ref = pages.to_reference();
        self.write(pages)?;
        let catalog_ref = self.write_object(Catalog { pages: pages_ref })?;

        let mut out = match self.out {
            Writer::Doc(w) => w,
            Writer::Stream(w) => w.end()?,
            Writer::Null => unreachable!(),
        };

        // xref
        let startxref = out.len;
        write_xref(&mut out)?;

        // trailer
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        #[serde(rename = "")]
        struct Info {
            producer: PdfString,
            #[serde(with = "serde_pdf::datetime")]
            creation_date: DateTime<Utc>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        #[serde(rename = "")]
        struct Trailer<'a> {
            size: usize,
            root: Reference<Catalog<'a>>,
            #[serde(rename = "ID")]
            id: (PdfStr<'a>, PdfStr<'a>),
            info: Info,
        }

        #[cfg(test)]
        let id = "test".to_string();
        #[cfg(not(test))]
        let id = Uuid::new_v4().to_string();

        write!(out, "trailer\n")?;
        to_writer(
            &mut out,
            &Trailer {
                size: self.id_seq.count() - 1,
                root: catalog_ref,
                id: (PdfStr::Hex(&id), PdfStr::Hex(&id)),
                info: Info {
                    producer: PdfString::Literal(format!(
                        "pdfrs v{} (github.com/rkusa/pdfrs)",
                        env!("CARGO_PKG_VERSION")
                    )),
                    #[cfg(not(test))]
                    creation_date: Utc::now(),
                    #[cfg(test)]
                    creation_date: Utc.ymd(2019, 6, 2).and_hms(14, 28, 0),
                },
            },
        )?;
        write!(out, "\nstartxref\n{}\n%%EOF", startxref)?;

        Ok(())
    }
}

impl IdSeq {
    pub fn next(&mut self) -> usize {
        let next = self.next_id;
        self.next_id += 1;
        next
    }

    pub fn count(&mut self) -> usize {
        self.next_id - 1
    }
}

fn write_xref<W: io::Write>(w: &mut DocWriter<W>) -> Result<(), io::Error> {
    write!(w, "xref\n")?;

    let mut from = 0;
    let mut to = 1;
    let mut offsets = Vec::with_capacity(w.xref.len());

    loop {
        if let Some(offset) = w.xref.remove(&to) {
            offsets.push(offset);
        } else {
            if from == 0 || !offsets.is_empty() {
                write!(w, "{} {}\n", from, to - from)?;

                if from == 0 {
                    write!(w, "0000000000 65535 f\n")?;
                }

                for offset in &offsets {
                    write!(w, "{:010} 00000 n\n", offset)?;
                }
            }

            if w.xref.is_empty() {
                break;
            }

            from = to + 1;
            offsets.clear();
        }

        to += 1;
    }

    Ok(())
}

pub struct DocWriter<W: io::Write> {
    w: W,
    len: usize,
    xref: HashMap<usize, usize>, // <object id, offset>
}

impl<W: io::Write> DocWriter<W> {
    fn new(w: W) -> Self {
        DocWriter {
            w,
            len: 0,
            xref: HashMap::new(),
        }
    }

    pub fn add_xref(&mut self, id: usize) {
        self.xref.insert(id, self.len);
    }
}

impl<W> io::Write for DocWriter<W>
where
    W: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let len = self.w.write(buf)?;
        self.len += len;
        Ok(len)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.w.flush()
    }
}

impl<W: io::Write> io::Write for Writer<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        match self {
            Writer::Doc(w) => w.write(buf),
            Writer::Stream(w) => w.write(buf),
            Writer::Null => unreachable!(),
        }
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        match self {
            Writer::Doc(w) => w.flush(),
            Writer::Stream(w) => w.flush(),
            Writer::Null => unreachable!(),
        }
    }
}

impl<W: io::Write> Writer<W> {
    fn into_stream(self, id_seq: &mut IdSeq) -> Self {
        match self {
            Writer::Doc(w) => Writer::Stream(Stream::new(id_seq, w)),
            s => s,
        }
    }

    fn end_stream(&mut self) -> Result<Option<Reference<StreamRef>>, io::Error> {
        let out = mem::replace(&mut *self, Writer::Null);
        match out {
            Writer::Doc(w) => {
                *self = Writer::Doc(w);
                Ok(None)
            }
            Writer::Stream(stream) => {
                let stream_ref = stream.to_reference();
                *self = Writer::Doc(stream.end()?);
                Ok(Some(stream_ref))
            }
            Writer::Null => unreachable!(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn xref_1() {
        let mut b = Vec::new();
        let mut w = DocWriter::new(&mut b);

        w.xref.insert(1, 3);
        w.xref.insert(2, 17);
        w.xref.insert(3, 81);
        w.xref.insert(4, 0);
        w.xref.insert(5, 331);
        w.xref.insert(6, 409);

        write_xref(&mut w).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../test/results/xref_1.txt"),
        );
    }

    #[test]
    fn xref_2() {
        let mut b = Vec::new();
        let mut w = DocWriter::new(&mut b);

        w.xref.insert(3, 25325);
        w.xref.insert(23, 25518);
        w.xref.insert(24, 25635);
        w.xref.insert(30, 25777);

        write_xref(&mut w).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../test/results/xref_2.txt"),
        );
    }

    #[test]
    fn basic() {
        use std::fs::File;

        let mut result = Vec::new();
        let doc = Document::new(&mut result).unwrap();
        doc.end().unwrap();

        let mut file =
            File::create("./test/results/basic.result.pdf").expect("Error creating result file");
        file.write_all(&result)
            .expect("Error writing result to file");

        let expected = include_bytes!("../test/results/basic.pdf");
        assert!(
            result.iter().eq(expected.iter()),
            "Resulting PDF does not match expected one"
        );
    }
}
