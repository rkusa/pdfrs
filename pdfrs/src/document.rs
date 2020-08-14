use std::collections::HashMap;
use std::io;
use std::mem;

use crate::fonts::{FontCollection, SubsetRef};
use crate::idseq::IdSeq;
use crate::page::{FontRef, Page, Pages, Resources};
use crate::stream::{to_async_writer, Stream, StreamRef};
use crate::writer::DocWriter;
use async_std::io::prelude::{Write, WriteExt};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_pdf::{Object, ObjectId, PdfStr, Reference};
use uuid::Uuid;

/// A type used to generate a PDF document.
pub struct Document<F: FontCollection, W> {
    page_stream: Option<Stream<W>>,
    pages_obj_id: ObjectId,
    pages: Vec<Reference<Page>>,
    id: String,
    creation_date: DateTime<Utc>,
    producer: String,
    page_state: PageState,
    font_collection: F,
    subsets: HashMap<F::FontRef, HashMap<SubsetRef, ObjectId>>,
}

pub struct DocumentBuilder<F: FontCollection> {
    id: Option<String>,
    creation_date: Option<DateTime<Utc>>,
    producer: Option<String>,
    font_collection: F,
}

#[derive(Default, Eq, PartialEq)]
pub(crate) struct PageState {
    fonts: HashMap<SubsetRef, Reference<FontRef>>,
    contents: Vec<Reference<StreamRef>>,
}

impl<F> Document<F, ()>
where
    F: FontCollection,
{
    pub fn builder(font_collection: F) -> DocumentBuilder<F> {
        DocumentBuilder::new(font_collection)
    }
}

impl<F, W> Document<F, W>
where
    F: FontCollection,
    W: Write + Unpin,
{
    /// Ends the current active page (if there is any), and adds the finished page to the document
    /// catalog.
    async fn end_page(&mut self) -> Result<DocWriter<W>, Error> {
        let page_stream = self.page_stream.take().ok_or(Error::StreamGone)?;

        let mut page_state = mem::take(&mut self.page_state);
        page_state.contents.push(page_stream.to_reference());
        let page = Page {
            parent: Reference::new(self.pages_obj_id.clone()),
            resources: Resources {
                font: page_state
                    .fonts
                    .into_iter()
                    .map(|(s, r)| (format!("F{}", s.font_id()), r))
                    .collect(),
            },
            contents: page_state.contents,
        };

        let mut doc = page_stream.end().await?;
        let page_ref = doc.serialize_object(page).await?;
        self.pages.push(page_ref);

        Ok(doc)
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

        let page_stream = self.page_stream.as_mut().ok_or(Error::StreamGone)?;
        let subset_refs = crate::text::write_text(text, font, page_stream).await?;

        for subset_ref in &subset_refs {
            if !subsets.contains_key(&subset_ref) {
                subsets.insert(*subset_ref, page_stream.reserve_object_id());
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
    pub async fn end(mut self) -> Result<(), Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Catalog {
            pages: Reference<Pages>,
        }

        let mut doc = self.end_page().await?;

        let Document {
            id,
            producer,
            font_collection,
            subsets,
            pages,
            ..
        } = self;

        // Write pages
        let pages_obj = Object::new(
            self.pages_obj_id.id(),
            self.pages_obj_id.rev(),
            Pages {
                media_box: (0.0, 0.0, 595.296, 841.896),
                count: pages.len(),
                kids: pages,
            },
        );
        let pages_ref = pages_obj.to_reference();
        doc.write_object(pages_obj).await?;
        let catalog_ref = doc.serialize_object(Catalog { pages: pages_ref }).await?;

        // Write fonts
        for (font_ref, subsets) in subsets {
            for (_, id) in subsets {
                font_collection
                    .write_objects(font_ref, id, &mut doc)
                    .await?;
            }
        }

        // xref
        let startxref = doc.len();
        doc.write_xref().await?;

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
            root: Reference<Catalog>,
            #[serde(rename = "ID")]
            id: (PdfStr<'a>, PdfStr<'a>),
            info: Info<'a>,
        }

        writeln!(doc, "trailer").await?;
        let size = doc.object_count() - 1;
        to_async_writer(
            &mut doc,
            &Trailer {
                size,
                root: catalog_ref,
                id: (PdfStr::Hex(&id), PdfStr::Hex(&id)),
                info: Info {
                    producer: PdfStr::Literal(&producer),
                    creation_date: &self.creation_date,
                },
            },
        )
        .await?;
        write!(doc, "\nstartxref\n{}\n%%EOF", startxref).await?;

        Ok(())
    }
}

impl<F> DocumentBuilder<F>
where
    F: FontCollection,
{
    pub fn new(font_collection: F) -> Self {
        DocumentBuilder {
            id: None,
            creation_date: None,
            producer: None,
            font_collection,
        }
    }

    /// Overrides the automatically generated PDF id by the provided `id`.
    pub fn with_id<S: Into<String>>(mut self, id: S) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Overrides the PDF's creation date (now by default) by the provided `date`.
    pub fn with_creation_date(mut self, date: DateTime<Utc>) -> Self {
        self.creation_date = Some(date);
        self
    }

    /// Overrides the default producer (pdfrs) by the provided `producer`.
    pub fn with_producer<S: Into<String>>(mut self, producer: S) -> Self {
        self.producer = Some(producer.into());
        self
    }

    /// Constructs a new `Document<'a, W>`.
    ///
    /// The document will immediately start generating a PDF. Each time the document is provided
    /// with further content, the resulting PDF output is generated right-away (most of the times).
    /// The resulting output is not buffered. It is directly written into the given `writer`. For
    /// most use-cases, it is thus recommended to provide a [`BufWriter`](std::io::BufWriter).
    pub async fn start<'a, W: Write + Unpin>(self, writer: W) -> Result<Document<F, W>, io::Error> {
        let mut wr = DocWriter::new(writer, IdSeq::new(1));

        // The PDF format mandates that we add at least 4 commented binary characters
        // (ASCII value >= 128), so that generic tools have a chance to detect
        // that it's a binary file
        write!(wr, "%PDF-1.6\n%").await?;
        wr.write_all(&[255, 255, 255, 255, b'\n', b'\n']).await?;

        Ok(Document {
            pages_obj_id: wr.reserve_object_id(),
            page_stream: Some(wr.start_stream().await?),
            pages: Vec::new(),
            id: self.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            creation_date: self.creation_date.unwrap_or_else(Utc::now),
            producer: self.producer.unwrap_or_else(|| {
                format!(
                    "pdfrs v{} (github.com/rkusa/pdfrs)",
                    env!("CARGO_PKG_VERSION")
                )
            }),
            page_state: PageState::default(),
            font_collection: self.font_collection,
            subsets: HashMap::new(),
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error writing PDF")]
    Io(#[from] io::Error),
    #[error("Error creating PDF object")]
    Pdf(#[from] serde_pdf::Error),
    #[error("Page stream gone (this is a bug, please report)")]
    StreamGone,
}
