use std::io::{self, Write};
use std::mem;

use crate::idseq::IdSeq;
use crate::page::{Page, Pages, Resources};
use crate::writer::{DocWriter, Writer};
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

/// A type used to generate a PDF document.
pub struct Document<'a, W: io::Write> {
    out: Writer<W>,
    id_seq: IdSeq,
    pages: Vec<Reference<Page<'a>>>,
}

impl<'a, W> Document<'a, W>
where
    W: io::Write,
{
    /// Constructs a new `Document<'a, W>`.
    ///
    /// The document will immediately start generating a PDF. Each time the document is provided
    /// with further content, the resulting PDF output is generated right-away (most of the times).
    /// The resulting output is not buffered. It is directly written into the given `writer`. For
    /// most use-cases, it is thus recommended to provide a [`BufWriter`](std::io::BufWriter).
    pub fn new(writer: W) -> Result<Self, io::Error> {
        let mut writer = DocWriter::new(writer);

        // The PDF format mandates that we add at least 4 commented binary characters
        // (ASCII value >= 128), so that generic tools have a chance to detect
        // that it's a binary file
        write!(writer, "%PDF-1.6\n%")?;
        writer.write_all(&[255, 255, 255, 255, '\n' as u8, '\n' as u8])?;

        Ok(Document {
            out: Writer::Doc(writer),
            id_seq: IdSeq::new(RESERVED_PAGES_ID + 1),
            pages: Vec::new(),
        })
    }

    /// Create a new PDF object with the given `content`.
    ///
    /// The document automatically assigns the next available object id to the new object (and a
    /// revision of `0`).
    fn new_object<D: Serialize>(&mut self, content: D) -> Object<D> {
        Object::new(self.id_seq.next(), 0, content)
    }

    /// Starts a new PDF stream object.
    ///
    /// If there is currently no PDF stream active, creates a new PDF stream object, writes its
    /// header and updates the document to write to that stream object until it is closed via
    /// [self.out.end_stream()].
    fn new_stream(&mut self) {
        let out = mem::replace(&mut self.out, Writer::Null);
        self.out = out.into_stream(&mut self.id_seq);
    }

    /// Creates new PDF object with the given `content`, intermediately writes it to the PDF
    /// output, and returns a reference to the written object.
    ///
    /// The document automatically assigns the next available object id to the new object (and a
    /// revision of `0`).
    fn write_object<D: Serialize>(&mut self, value: D) -> Result<Reference<D>, serde_pdf::Error> {
        let obj = self.new_object(value);
        let r = obj.to_reference();
        self.write(obj)?;
        return Ok(r);
    }

    /// Writes the provided `object` to the PDF output.
    fn write<D: Serialize>(&mut self, object: Object<D>) -> Result<(), serde_pdf::Error> {
        match self.out {
            Writer::Stream(_) => {
                // TODO: maybe close stream instead of panicing
                unreachable!();
            }
            Writer::Doc(ref mut w) => {
                w.add_xref(object.id());
                serde_pdf::to_writer(w, &object)
            }
            Writer::Null => {
                unreachable!();
            }
        }
    }

    /// Starts a new PDF page, and starts the page stream.
    fn start_page(&mut self) {
        self.new_stream();
    }

    /// Ends the current active page (if there is any), and adds the finished page to the document
    /// catalog.
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

    /// Ends the document.
    ///
    /// This writes all the document's metadata and page reference to the PDF output. The document's
    /// `writer` contains a valid PDF afterwards.
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
        let startxref = out.len();
        out.write_xref()?;

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

#[cfg(test)]
mod test {
    use super::*;

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
