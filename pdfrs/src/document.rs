use std::collections::HashMap;
use std::io;
use std::mem;

use crate::fonts::{Font, FontObject};
use crate::idseq::IdSeq;
use crate::page::{Page, Pages, Resources};
use crate::stream::{to_async_writer, StreamRef};
use crate::writer::{DocWriter, Writer};
use async_std::io::prelude::{Write, WriteExt};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_pdf::{Object, ObjectId, PdfStr, Reference};
use thiserror::Error;
use uuid::Uuid;

const RESERVED_PAGES_ID: usize = 1;
// const PAGES_ID: ObjectId = ObjectId::new(RESERVED_PAGES_ID, 0);
// const PAGES_REFERENCE: Reference<Pages<'_>> = Reference::new(PAGES_ID);

/// A type used to generate a PDF document.
pub struct Document<'a, W: Write> {
    out: Writer<W>,
    id_seq: IdSeq,
    pages: Vec<Reference<Page<'a>>>,
    id: String,
    creation_date: DateTime<Utc>,
    producer: String,
    fonts: HashMap<&'a str, FontEntry<'a>>,
    page_state: PageState<'a>,
}

#[derive(Eq, PartialEq, Hash)]
struct FontEntry<'a> {
    id: usize,
    reference: Reference<FontObject<'a>>,
}

#[derive(Default, Eq, PartialEq)]
struct PageState<'a> {
    fonts: HashMap<usize, Reference<FontObject<'a>>>,
    contents: Vec<Reference<StreamRef>>,
}

impl<'a, W> Document<'a, W>
where
    W: Write + Unpin,
{
    /// Constructs a new `Document<'a, W>`.
    ///
    /// The document will immediately start generating a PDF. Each time the document is provided
    /// with further content, the resulting PDF output is generated right-away (most of the times).
    /// The resulting output is not buffered. It is directly written into the given `writer`. For
    /// most use-cases, it is thus recommended to provide a [`BufWriter`](std::io::BufWriter).
    pub async fn new(writer: W) -> Result<Document<'a, W>, io::Error> {
        let mut writer = DocWriter::new(writer);

        // The PDF format mandates that we add at least 4 commented binary characters
        // (ASCII value >= 128), so that generic tools have a chance to detect
        // that it's a binary file
        write!(writer, "%PDF-1.6\n%").await?;
        writer
            .write_all(&[255, 255, 255, 255, b'\n', b'\n'])
            .await?;

        Ok(Document {
            out: Writer::Doc(writer),
            id_seq: IdSeq::new(RESERVED_PAGES_ID + 1),
            pages: Vec::new(),
            id: Uuid::new_v4().to_string(),
            creation_date: Utc::now(),
            producer: format!(
                "pdfrs v{} (github.com/rkusa/pdfrs)",
                env!("CARGO_PKG_VERSION")
            ),
            fonts: HashMap::new(),
            page_state: PageState::default(),
        })
    }

    /// Overrides the automatically generated PDF id by the provided `id`.
    pub fn set_id<S: Into<String>>(&mut self, id: S) {
        self.id = id.into();
    }

    /// Overrides the PDF's creation date (now by default) by the provided `date`.
    pub fn set_creation_date(&mut self, date: DateTime<Utc>) {
        self.creation_date = date;
    }

    /// Overrides the default producer (pdfrs) by the provided `producer`.
    pub fn set_producer<S: Into<String>>(&mut self, producer: S) {
        self.producer = producer.into();
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
    async fn new_stream(&mut self) -> Result<(), io::Error> {
        let out = mem::replace(&mut self.out, Writer::Null);
        self.out = out.into_stream(&mut self.id_seq).await?;
        Ok(())
    }

    /// Creates new PDF object with the given `content`, intermediately writes it to the PDF
    /// output, and returns a reference to the written object.
    ///
    /// The document automatically assigns the next available object id to the new object (and a
    /// revision of `0`).
    async fn write_object<D: Serialize>(
        &mut self,
        value: D,
    ) -> Result<Reference<D>, serde_pdf::Error> {
        let obj = self.new_object(value);
        let r = obj.to_reference();
        self.write(obj).await?;
        Ok(r)
    }

    /// Writes the provided `object` to the PDF output.
    async fn write<D: Serialize>(&mut self, object: Object<D>) -> Result<(), serde_pdf::Error> {
        match self.out {
            Writer::Stream(_) => {
                // FIXME: maybe close stream instead of panicing
                unreachable!();
            }
            Writer::Doc(ref mut w) => {
                w.add_xref(object.id());
                to_async_writer(w, &object).await
            }
            Writer::Null => {
                unreachable!();
            }
        }
    }

    /// Starts a new PDF page, and starts the page stream.
    async fn start_page(&mut self) -> Result<(), io::Error> {
        self.new_stream().await
    }

    /// Ends the current active page (if there is any), and adds the finished page to the document
    /// catalog.
    async fn end_page(&mut self) -> Result<(), serde_pdf::Error> {
        if let Some(content_ref) = self.out.end_stream().await? {
            // TODO: move to consts once const_fn landed
            let id = ObjectId::new(RESERVED_PAGES_ID, 0);
            let reference: Reference<Pages<'_>> = Reference::new(id);

            let mut page_state = mem::take(&mut self.page_state);
            page_state.contents.push(content_ref);

            let page = Page {
                parent: reference,
                resources: Resources {
                    // while obsolete since PDF 1.4, still here for compatibility reasons, and simply
                    // setting all possible values ...
                    proc_set: vec!["PDF", "Text", "ImageB", "ImageC", "ImageI"],
                    font: page_state
                        .fonts
                        .into_iter()
                        .map(|(id, font_ref)| (format!("F{}", id), font_ref))
                        .collect(),
                },
                contents: page_state.contents,
            };

            let page_ref = self.write_object(page).await?;
            self.pages.push(page_ref);
        }

        Ok(())
    }

    pub async fn text(&mut self, text: &str, font: &'a Font) -> Result<(), Error> {
        if !self.fonts.contains_key(font.base_name()) {
            if let Some(content_ref) = self.out.end_stream().await? {
                self.page_state.contents.push(content_ref);
            }

            let font_object = font.object();
            let font_ref = self.write_object(font_object).await?;
            self.fonts.insert(
                font.base_name(),
                FontEntry {
                    id: self.fonts.len(),
                    reference: font_ref,
                },
            );

            self.new_stream().await?;
        }

        if let Some(font_entry) = self.fonts.get(font.base_name()) {
            self.page_state
                .fonts
                .entry(font_entry.id)
                .or_insert_with(|| font_entry.reference.clone());

            match &mut self.out {
                Writer::Stream(ref mut s) => {
                    crate::text::write_text(s, text, font_entry.id, font).await?;
                }
                Writer::Doc(_) | Writer::Null => {
                    // FIXME: return error instead, or ignore and do nothing
                    unreachable!();
                }
            }
        }

        Ok(())
    }

    /// Ends the document.
    ///
    /// This writes all the document's metadata and page reference to the PDF output. The document's
    /// `writer` contains a valid PDF afterwards.
    pub async fn end(mut self) -> Result<(), serde_pdf::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Catalog<'a> {
            pages: Reference<Pages<'a>>,
        }

        if self.pages.is_empty() {
            self.start_page().await?;
        }

        self.end_page().await?;

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
        self.write(pages).await?;
        let catalog_ref = self.write_object(Catalog { pages: pages_ref }).await?;

        let mut out = match self.out {
            Writer::Doc(w) => w,
            Writer::Stream(w) => w.end().await?,
            Writer::Null => unreachable!(),
        };

        // xref
        let startxref = out.len();
        out.write_xref().await?;

        // trailer
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        #[serde(rename = "")]
        struct Info<'a> {
            producer: PdfStr<'a>,
            #[serde(with = "serde_pdf::datetime")]
            creation_date: &'a DateTime<Utc>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        #[serde(rename = "")]
        struct Trailer<'a> {
            size: usize,
            root: Reference<Catalog<'a>>,
            #[serde(rename = "ID")]
            id: (PdfStr<'a>, PdfStr<'a>),
            info: Info<'a>,
        }

        writeln!(out, "trailer").await?;
        to_async_writer(
            &mut out,
            &Trailer {
                size: self.id_seq.count() - 1,
                root: catalog_ref,
                id: (PdfStr::Hex(&self.id), PdfStr::Hex(&self.id)),
                info: Info {
                    producer: PdfStr::Literal(&self.producer),
                    creation_date: &self.creation_date,
                },
            },
        )
        .await?;
        write!(out, "\nstartxref\n{}\n%%EOF", startxref).await?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("error writing PDF")]
    Io(#[from] io::Error),
    #[error("error creating PDF object")]
    Pdf(#[from] serde_pdf::Error),
}
