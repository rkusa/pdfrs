use std::collections::HashMap;
use std::{io, mem};

use crate::idseq::IdSeq;
use crate::stream::{to_async_writer, Stream, StreamRef};
use async_std::io::prelude::{Write, WriteExt};
use async_std::io::BufWriter;
use async_std::task::Context;
use async_std::task::Poll;
use pin_project::pin_project;
use serde::Serialize;
use serde_pdf::{Object, Reference};
use std::pin::Pin;

/// A type that is always [`Write`](async_std::io::Write), but either contains a `DocWriter<W>` or a
/// [`Stream`](stream::Stream).
#[pin_project(project = WriterProj)]
pub enum Writer<W: Write> {
    Doc(#[pin] DocWriter<W>),
    Stream(#[pin] Stream<W>),
    Null,
}

/// A type that keeps track of a PDF XREF table while forwarding writes to its wrapped writer.
///
/// It keeps track of how many bytes have already been written to correctly reference objects
/// inside the document.
#[pin_project]
pub struct DocWriter<W: Write> {
    #[pin]
    w: BufWriter<W>,
    len: usize,
    xref: HashMap<usize, usize>, // <object id, offset>
}

impl<W: Write + Unpin> Writer<W> {
    /// Converts the current variant into a `Writer::Stream`.
    pub async fn into_stream(self, id_seq: &mut IdSeq) -> Result<Writer<W>, io::Error> {
        Ok(match self {
            Writer::Doc(w) => Writer::Stream(Stream::new(id_seq, w).await?),
            s => s,
        })
    }

    /// Ends the current `Stream<W>`, if it is currently one. Returns a reference to the ended
    /// stream.
    pub async fn end_stream(&mut self) -> Result<Option<Reference<StreamRef>>, io::Error> {
        let out = mem::replace(&mut *self, Writer::Null);
        match out {
            Writer::Doc(w) => {
                *self = Writer::Doc(w);
                Ok(None)
            }
            Writer::Stream(stream) => {
                let stream_ref = stream.to_reference();
                *self = Writer::Doc(stream.end().await?);
                Ok(Some(stream_ref))
            }
            Writer::Null => unreachable!(),
        }
    }
}

impl<W: Write + Unpin> DocWriter<W> {
    /// Constructs a new `DocWriter<W>` wrapping the given writer.
    pub fn new(w: W) -> Self {
        DocWriter {
            w: BufWriter::new(w),
            len: 0,
            xref: HashMap::new(),
        }
    }

    /// The length in bytes of the already written PDF output.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Add an XREF entry for the current position of the PDF output and assign it to the provided
    /// `id`.
    pub fn add_xref(&mut self, id: usize) {
        self.xref.insert(id, self.len);
    }

    /// Writes the provided `object` to the PDF output.
    pub async fn write_object<D: Serialize>(
        &mut self,
        object: Object<D>,
    ) -> Result<(), serde_pdf::Error> {
        self.add_xref(object.id());
        to_async_writer(self, &object).await
    }

    /// Writes the XREF table into into the wrapped writer of the `DocWriter<W>`.
    pub async fn write_xref(&mut self) -> Result<(), io::Error> {
        writeln!(self.w, "xref").await?;

        let mut from = 0;
        let mut to = 1;
        let mut offsets = Vec::with_capacity(self.xref.len());

        loop {
            if let Some(offset) = self.xref.remove(&to) {
                offsets.push(offset);
            } else {
                if from == 0 || !offsets.is_empty() {
                    writeln!(self.w, "{} {}", from, to - from).await?;

                    if from == 0 {
                        writeln!(self.w, "0000000000 65535 f").await?;
                    }

                    for offset in &offsets {
                        writeln!(self.w, "{:010} 00000 n", offset).await?;
                    }
                }

                if self.xref.is_empty() {
                    break;
                }

                from = to + 1;
                offsets.clear();
            }

            to += 1;
        }

        Ok(())
    }
}

impl<W: Write + Unpin> Write for Writer<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.project() {
            WriterProj::Doc(w) => w.poll_write(cx, buf),
            WriterProj::Stream(w) => w.poll_write(cx, buf),
            WriterProj::Null => unreachable!(),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.project() {
            WriterProj::Doc(w) => w.poll_flush(cx),
            WriterProj::Stream(w) => w.poll_flush(cx),
            WriterProj::Null => unreachable!(),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.project() {
            WriterProj::Doc(w) => w.poll_close(cx),
            WriterProj::Stream(w) => w.poll_close(cx),
            WriterProj::Null => unreachable!(),
        }
    }
}

impl<W: Write + Unpin> Write for DocWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let project = self.project();
        match project.w.poll_write(cx, buf) {
            Poll::Ready(result) => {
                let len = result?;
                *project.len += len;
                Poll::Ready(Ok(len))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().w.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().w.poll_close(cx)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[async_std::test]
    async fn xref_1() {
        let mut b = Vec::new();
        let mut w = DocWriter::new(&mut b);

        w.xref.insert(1, 3);
        w.xref.insert(2, 17);
        w.xref.insert(3, 81);
        w.xref.insert(4, 0);
        w.xref.insert(5, 331);
        w.xref.insert(6, 409);

        w.write_xref().await.unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../tests/fixtures/xref_1.txt"),
        );
    }

    #[async_std::test]
    async fn xref_2() {
        let mut b = Vec::new();
        let mut w = DocWriter::new(&mut b);

        w.xref.insert(3, 25325);
        w.xref.insert(23, 25518);
        w.xref.insert(24, 25635);
        w.xref.insert(30, 25777);

        w.write_xref().await.unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../tests/fixtures/xref_2.txt"),
        );
    }
}
