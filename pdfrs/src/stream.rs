use std::collections::HashSet;
use std::io;
use std::pin::Pin;

use crate::fonts::{Font, SubsetRef};
use crate::idseq::IdSeq;
use crate::writer::DocWriter;
use async_std::io::{prelude::WriteExt, Write};
use async_std::task::Context;
use async_std::task::Poll;
use pin_project::pin_project;
use serde::Serialize;
use serde_pdf::{Object, ObjectId, Reference};

/// A type used to handle writing a PDF stream to a PDF document. It handles creating a
/// corresponding PDF object, keeping track of the stream's length as well as writing the stream
/// it all it's related meta data to the PDF document.
#[pin_project]
pub struct Stream<W: Write> {
    id: ObjectId,
    len_obj_id: ObjectId,
    len: usize,
    #[pin]
    wr: DocWriter<W>,
    prev_subset: Option<SubsetRef>,
}

/// The properties of a PDF stream's PDF object.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "")]
pub struct StreamMeta {
    pub length: Reference<usize>,
}

/// A type used to create PDF references (`Reference<StreamRef>`).
pub type StreamRef = ();

impl<W: Write + Unpin> Stream<W> {
    /// Constructs a new PDF stream.
    pub async fn new(id_seq: &mut IdSeq, wr: DocWriter<W>) -> Result<Stream<W>, io::Error> {
        let mut stream = Stream {
            id: ObjectId::new(id_seq.next(), 0),
            len_obj_id: ObjectId::new(id_seq.next(), 0),
            len: 0,
            wr,
            prev_subset: None,
        };
        stream.write_header().await?;
        Ok(stream)
    }

    /// Returns a PDF reference to the stream's PDF object.
    pub fn to_reference(&self) -> Reference<StreamRef> {
        Reference::new(self.id.clone())
    }

    /// Writes the stream's and its corresponding object's start markers, as well as writing its
    /// object properties (which includes a reference to its length object).
    async fn write_header(&mut self) -> Result<(), io::Error> {
        self.wr.add_xref(self.id.id());
        writeln!(self.wr, "{} {} obj", self.id.id(), self.id.rev()).await?;
        to_async_writer(
            &mut self.wr,
            &StreamMeta {
                length: Reference::new(self.len_obj_id.clone()),
            },
        )
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        writeln!(self.wr, "\nstream").await?;

        Ok(())
    }

    /// Ends the PDF stream, which involves writing the stream's and corresponding object's end
    /// markers and the stream's length object.
    pub async fn end(mut self) -> Result<DocWriter<W>, io::Error> {
        writeln!(self.wr, "endstream\nendobj\n").await?;

        self.wr.add_xref(self.len_obj_id.id());
        let len_obj = Object::new(self.len_obj_id.id(), self.len_obj_id.rev(), self.len);
        to_async_writer(&mut self.wr, &len_obj)
            .await
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        Ok(self.wr)
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

impl<W: Write + Unpin> Write for Stream<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let project = self.project();
        match project.wr.poll_write(cx, buf) {
            Poll::Ready(result) => {
                let len = result?;
                *project.len += len;
                Poll::Ready(Ok(len))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().wr.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().wr.poll_close(cx)
    }
}

pub async fn to_async_writer<W, T>(mut w: W, value: &T) -> Result<(), serde_pdf::Error>
where
    W: async_std::io::Write + Unpin,
    T: Serialize,
{
    let s = serde_pdf::to_string(value)?;
    w.write_all(s.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fonts::HELVETICA;
    use crate::idseq::IdSeq;

    #[async_std::test]
    async fn test_position_glyphs() {
        let mut buf = Vec::new();
        let mut id_seq = IdSeq::new(0);
        let mut stream = Stream::new(&mut id_seq, DocWriter::new(&mut buf))
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
