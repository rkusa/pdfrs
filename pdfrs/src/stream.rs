use std::collections::HashSet;
use std::io;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

use crate::fonts::{Font, SubsetRef};
use crate::writer::DocWriter;
use async_compression::futures::write::ZlibEncoder;
use async_std::io::prelude::WriteExt;
use async_std::task::Context;
use async_std::task::Poll;
use futures_io::AsyncWrite;
use pin_project::pin_project;
use serde::Serialize;
use serde_pdf::{Object, ObjectId, Reference};

/// A type used to handle writing a PDF stream to a PDF document. It handles creating a
/// corresponding PDF object, keeping track of the stream's length as well as writing the stream
/// it all it's related meta data to the PDF document.
#[pin_project]
pub struct Stream<W> {
    id: ObjectId,
    len_obj_id: ObjectId,
    len1_obj_id: Option<ObjectId>,
    len1: usize,
    doc_len_before: usize,
    #[pin]
    wr: StreamInner<W>,
    prev_subset: Option<SubsetRef>,
}

#[pin_project(project = StreamInnerProj)]
enum StreamInner<W> {
    Doc(#[pin] DocWriter<W>),
    Deflate(#[pin] ZlibEncoder<DocWriter<W>>),
}

/// The properties of a PDF stream's PDF object.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "")]
struct StreamMeta {
    length: Reference<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    length1: Option<Reference<usize>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    filter: Vec<Filter>,
}

#[derive(Serialize)]
enum Filter {
    FlateDecode,
    // TODO: ASCII85Decode,
}

/// A type used to create PDF references (`Reference<StreamRef>`).
pub type StreamRef = ();

impl<W: AsyncWrite + Unpin> Stream<W> {
    /// Constructs a new PDF stream.
    pub async fn start(
        mut wr: DocWriter<W>,
        compresse: bool,
        with_len1: bool,
    ) -> Result<Stream<W>, io::Error> {
        let id = wr.reserve_object_id();
        let len_obj_id = wr.reserve_object_id();
        let len1_obj_id = if compresse && with_len1 {
            Some(wr.reserve_object_id())
        } else {
            None
        };

        wr.add_xref(id.id());
        writeln!(wr, "{} {} obj", id.id(), id.rev()).await?;
        to_async_writer(
            &mut wr,
            &StreamMeta {
                length: Reference::new(len_obj_id.clone()),
                length1: len1_obj_id.clone().map(Reference::new),
                filter: if compresse {
                    vec![Filter::FlateDecode]
                } else {
                    Vec::new()
                },
            },
        )
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        writeln!(wr, "\nstream").await?;

        Ok(Stream {
            id,
            len_obj_id,
            len1_obj_id,
            len1: 0,
            doc_len_before: wr.len(),
            wr: if compresse {
                StreamInner::Deflate(ZlibEncoder::new(wr))
            } else {
                StreamInner::Doc(wr)
            },
            prev_subset: None,
        })
    }

    /// Returns a PDF reference to the stream's PDF object.
    pub fn to_reference(&self) -> Reference<StreamRef> {
        Reference::new(self.id.clone())
    }

    pub fn reserve_object_id(&mut self) -> ObjectId {
        match &mut self.wr {
            StreamInner::Doc(d) => d.reserve_object_id(),
            StreamInner::Deflate(d) => d.get_mut().reserve_object_id(),
        }
    }

    /// Ends the PDF stream, which involves writing the stream's and corresponding object's end
    /// markers and the stream's length object.
    pub async fn end(mut self) -> Result<DocWriter<W>, io::Error> {
        self.flush().await?;
        let len = self.wr.len() - self.doc_len_before;
        let mut wr = self.wr.into_inner();
        writeln!(wr, "\nendstream\nendobj\n").await?;

        wr.add_xref(self.len_obj_id.id());
        let len_obj = Object::new(self.len_obj_id.id(), self.len_obj_id.rev(), len);
        to_async_writer(&mut wr, &len_obj)
            .await
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        if let Some(len1_obj_id) = self.len1_obj_id {
            let len1_obj = Object::new(len1_obj_id.id(), len1_obj_id.rev(), self.len1);
            to_async_writer(&mut wr, &len1_obj)
                .await
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        }

        Ok(wr)
    }

    /// Begins a text object (BT - PDF spec 1.7 page 405).
    pub async fn begin_text(&mut self) -> Result<(), io::Error> {
        // FIXME: move text operations into an object returned here to prevent nested BT.
        writeln!(self, "BT").await
    }

    /// Ends a text object (ET - PDF spec 1.7 page 405).
    pub async fn end_text(&mut self) -> Result<(), io::Error> {
        writeln!(self, "ET").await
    }

    /// Sets the text matrix (Tm - PDF spec 1.7 page 406).
    #[allow(clippy::many_single_char_names)]
    pub async fn set_text_matrix(
        &mut self,
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
        f: f64,
    ) -> Result<(), io::Error> {
        writeln!(
            self,
            "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} Tm",
            a, b, c, d, e, f
        )
        .await
    }

    /// Sets the text leading (TL - PDF spec 1.7 page 398).
    pub async fn set_text_leading(&mut self, leading: f64) -> Result<(), io::Error> {
        writeln!(self, "{:.3} TL", leading).await
    }

    /// Sets the text font and font size (Tf - PDF spec 1.7 page 398).
    pub async fn set_text_font(&mut self, font_id: usize, size: f64) -> Result<(), io::Error> {
        writeln!(self, "/F{} {:.3} Tf", font_id, size).await
    }

    // Sets the color to use for non-stroking operations (sc - PDF spec 1.7 page 287).
    pub async fn set_fill_color(&mut self, c1: f64, c2: f64, c3: f64) -> Result<(), io::Error> {
        writeln!(self, "{:.3} {:.3} {:.3} sc", c1, c2, c3).await
    }

    pub async fn show_text_string(
        &mut self,
        text: &str,
        font: &dyn Font,
        size: f64,
    ) -> Result<HashSet<SubsetRef>, io::Error> {
        let mut subset_refs = HashSet::with_capacity(1);
        let mut prev = None;
        let mut offset = 0;
        for (i, c) in text.char_indices() {
            if let Some(kerning) = prev.and_then(|p| font.kerning(p, c)) {
                let srfs = self.write_text(&text[offset..i], font, size).await?;
                subset_refs.extend(srfs);
                write!(self, " {} ", -kerning).await?;
                offset = i;
            }
            prev = Some(c);
        }
        if offset < text.len() {
            let srfs = self.write_text(&text[offset..], font, size).await?;
            subset_refs.extend(srfs);
        }

        writeln!(self, "] TJ").await?;
        self.prev_subset = None;
        Ok(subset_refs)
    }

    async fn write_text(
        &mut self,
        text: &str,
        font: &dyn Font,
        size: f64,
    ) -> Result<HashSet<SubsetRef>, io::Error> {
        let mut subset_refs = HashSet::with_capacity(1);

        // TODO: re-use buffer for other method calls?
        let mut buf = Vec::with_capacity(text.len());
        let mut offset = 0;
        loop {
            let substr = &text[offset..];
            let (subset_ref, n) = font.encode_into(substr, &mut buf)?;
            if self.prev_subset != Some(subset_ref) {
                if self.prev_subset.is_some() {
                    writeln!(self, "] TJ").await?
                }
                self.set_text_font(subset_ref.font_id(), size).await?;
                write!(self, "[").await?;
            }

            self.write_all(&buf).await?;
            subset_refs.insert(subset_ref);
            self.prev_subset = Some(subset_ref);
            if n < substr.len() {
                offset += n;
                buf.clear();
            } else {
                break;
            }
        }

        Ok(subset_refs)
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for Stream<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let project = self.project();
        let poll = match project.wr.project() {
            StreamInnerProj::Doc(w) => w.poll_write(cx, buf),
            StreamInnerProj::Deflate(w) => w.poll_write(cx, buf),
        };
        match poll {
            Poll::Ready(result) => {
                let len = result?;
                *project.len1 += len;
                Poll::Ready(Ok(len))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.project().wr.project() {
            StreamInnerProj::Doc(w) => w.poll_flush(cx),
            StreamInnerProj::Deflate(w) => w.poll_flush(cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.project().wr.project() {
            StreamInnerProj::Doc(w) => w.poll_close(cx),
            StreamInnerProj::Deflate(w) => w.poll_close(cx),
        }
    }
}

pub async fn to_async_writer<W, T>(mut w: W, value: &T) -> Result<(), serde_pdf::Error>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let s = serde_pdf::to_string(value)?;
    w.write_all(s.as_bytes()).await?;
    Ok(())
}

impl<W> StreamInner<W>
where
    DocWriter<W>: AsyncWrite,
{
    fn into_inner(self) -> DocWriter<W> {
        match self {
            StreamInner::Doc(d) => d,
            StreamInner::Deflate(d) => d.into_inner(),
        }
    }
}

impl<W> Deref for StreamInner<W>
where
    DocWriter<W>: AsyncWrite,
{
    type Target = DocWriter<W>;

    fn deref(&self) -> &Self::Target {
        match &self {
            StreamInner::Doc(d) => d,
            StreamInner::Deflate(d) => d.get_ref(),
        }
    }
}

impl<W> DerefMut for StreamInner<W>
where
    DocWriter<W>: AsyncWrite,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            StreamInner::Doc(d) => d,
            StreamInner::Deflate(d) => d.get_mut(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fonts::HELVETICA;
    use crate::idseq::IdSeq;

    #[async_std::test]
    async fn test_position_glyphs() {
        let mut buf = Vec::new();
        let mut stream = DocWriter::new(&mut buf, IdSeq::new(1))
            .start_stream(false)
            .await
            .unwrap();

        let len_before = stream.wr.len();
        stream
            .show_text_string("Hello World", &&*HELVETICA, 12.0)
            .await
            .unwrap();
        assert_eq!(
            &String::from_utf8_lossy(&buf[len_before..]),
            "/F0 12.000 Tf\n[(Hello W) 30 (or) -15 (ld)] TJ\n"
        );
    }
}
