use std::collections::HashMap;
use std::io;
use std::mem;

use crate::fonts::{FontCollection, SubsetRef};
use crate::idseq::IdSeq;
use crate::page::{FontRef, Page, Pages, Resources};
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
pub struct Document<'a, F: FontCollection, W: Write> {
    out: Writer<W>,
    id_seq: IdSeq,
    pages: Vec<Reference<Page<'a>>>,
    id: String,
    creation_date: DateTime<Utc>,
    producer: String,
    page_state: PageState,
    font_collection: F,
    subsets: HashMap<F::FontRef, HashMap<SubsetRef, ObjectId>>,
}

#[derive(Default, Eq, PartialEq)]
struct PageState {
    fonts: HashMap<SubsetRef, Reference<FontRef>>,
    contents: Vec<Reference<StreamRef>>,
}

impl<'a, F, W> Document<'a, F, W>
where
    F: FontCollection,
    W: Write + Unpin,
{
    /// Constructs a new `Document<'a, W>`.
    ///
    /// The document will immediately start generating a PDF. Each time the document is provided
    /// with further content, the resulting PDF output is generated right-away (most of the times).
    /// The resulting output is not buffered. It is directly written into the given `writer`. For
    /// most use-cases, it is thus recommended to provide a [`BufWriter`](std::io::BufWriter).
    pub async fn new(fonts: F, writer: W) -> Result<Document<'a, F, W>, io::Error> {
        let mut writer = DocWriter::new(writer);

        // The PDF format mandates that we add at least 4 commented binary characters
        // (ASCII value >= 128), so that generic tools have a chance to detect
        // that it's a binary file
        write!(writer, "%PDF-1.6\n%").await?;
        writer
            .write_all(&[255, 255, 255, 255, b'\n', b'\n'])
            .await?;

        let mut doc = Document {
            out: Writer::Doc(writer),
            id_seq: IdSeq::new(RESERVED_PAGES_ID + 1),
            pages: Vec::new(),
            id: Uuid::new_v4().to_string(),
            creation_date: Utc::now(),
            producer: format!(
                "pdfrs v{} (github.com/rkusa/pdfrs)",
                env!("CARGO_PKG_VERSION")
            ),
            page_state: PageState::default(),
            font_collection: fonts,
            subsets: HashMap::new(),
        };
        doc.start_page().await?;
        Ok(doc)
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
            Writer::Doc(ref mut w) => w.write_object(object).await,
            Writer::Null => unreachable!(),
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
                        .map(|(s, r)| (format!("F{}", s.font_id()), r))
                        .collect(),
                },
                contents: page_state.contents,
            };

            let page_ref = self.write_object(page).await?;
            self.pages.push(page_ref);
        }

        Ok(())
    }

    pub async fn text(&mut self, text: &str, font_ref: Option<F::FontRef>) -> Result<(), Error> {
        if text.is_empty() {
            return Ok(());
        }

        let font_ref = font_ref.unwrap_or_default();
        let font = self.font_collection.font(font_ref);
        let subsets = self
            .subsets
            .entry(font_ref)
            .or_insert_with(Default::default);

        let subset_refs = match &mut self.out {
            Writer::Stream(ref mut s) => crate::text::write_text(text, font, s).await?,
            Writer::Doc(_) | Writer::Null => {
                // FIXME: return error instead, or ignore and do nothing
                unreachable!();
            }
        };

        for subset_ref in &subset_refs {
            if !subsets.contains_key(&subset_ref) {
                subsets.insert(*subset_ref, ObjectId::new(self.id_seq.next(), 0));
            }
        }
        self.page_state.fonts.extend(
            subset_refs
                .into_iter()
                .filter_map(|s| subsets.get(&s).map(|o| (s, Reference::new(o.clone())))),
        );

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

        // Write pages
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

        let Document {
            out,
            mut id_seq,
            id,
            producer,
            font_collection,
            subsets,
            ..
        } = self;

        let mut out = match out {
            Writer::Doc(w) => w,
            Writer::Stream(w) => w.end().await?,
            Writer::Null => unreachable!(),
        };

        // Write fonts
        for (font_ref, subsets) in subsets {
            let font = font_collection.font(font_ref);
            for (_, id) in subsets {
                let font_obj = Object::new(id.id(), id.rev(), font.object());
                out.write_object(font_obj).await?;
            }
        }

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
                size: id_seq.count() - 1,
                root: catalog_ref,
                id: (PdfStr::Hex(&id), PdfStr::Hex(&id)),
                info: Info {
                    producer: PdfStr::Literal(&producer),
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
