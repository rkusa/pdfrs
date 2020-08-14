use std::collections::HashMap;
use std::io;

use crate::idseq::IdSeq;
use crate::stream::{to_async_writer, Stream};
use async_std::io::prelude::{Write, WriteExt};
use async_std::io::BufWriter;
use async_std::task::Context;
use async_std::task::Poll;
use pin_project::pin_project;
use serde::Serialize;
use serde_pdf::{Object, ObjectId, Reference};
use std::pin::Pin;

/// A type that keeps track of a PDF XREF table while forwarding writes to its wrapped writer.
///
/// It keeps track of how many bytes have already been written to correctly reference objects
/// inside the document.
#[pin_project]
pub struct DocWriter<W> {
    #[pin]
    w: BufWriter<W>,
    len: usize,
    id_seq: IdSeq,
    xref: HashMap<usize, usize>, // <object id, offset>
}

impl<W: Write + Unpin> DocWriter<W> {
    /// Constructs a new `DocWriter<W>` wrapping the given writer.
    pub fn new(w: W, id_seq: IdSeq) -> Self {
        DocWriter {
            w: BufWriter::new(w),
            len: 0,
            xref: HashMap::new(),
            id_seq,
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

    pub fn reserve_object_id(&mut self) -> ObjectId {
        ObjectId::new(self.id_seq.next(), 0)
    }

    pub fn object_count(&mut self) -> usize {
        self.id_seq.count()
    }

    /// Create a new PDF object with the given `content`.
    ///
    /// The document automatically assigns the next available object id to the new object (and a
    /// revision of `0`).
    fn new_object<D: Serialize>(&mut self, content: D) -> Object<D> {
        Object::new(self.id_seq.next(), 0, content)
    }

    /// Creates new PDF object with the given `content`, intermediately writes it to the PDF
    /// output, and returns a reference to the written object.
    ///
    /// The document automatically assigns the next available object id to the new object (and a
    /// revision of `0`).
    pub async fn serialize_object<D: Serialize>(
        &mut self,
        value: D,
    ) -> Result<Reference<D>, serde_pdf::Error> {
        let obj = self.new_object(value);
        let r = obj.to_reference();
        self.write_object(obj).await?;
        Ok(r)
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

    pub async fn start_stream(self) -> Result<Stream<W>, io::Error> {
        Stream::start(self).await
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
        let mut w = DocWriter::new(&mut b, IdSeq::new(0));

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
        let mut w = DocWriter::new(&mut b, IdSeq::new(0));

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
