use std::collections::HashMap;
use std::{io, mem};

use crate::idseq::IdSeq;
use crate::stream::{Stream, StreamRef};
use serde_pdf::Reference;

/// A type that is always [`Write`](std::io::Write), but either contains a `DocWriter<W>` or a
/// [`Stream`](stream::Stream).
pub enum Writer<W: io::Write> {
    Doc(DocWriter<W>),
    Stream(Stream<W>),
    Null,
}

/// A type that keeps track of a PDF XREF table while forwarding writes to its wrapped writer.
///
/// It keeps track of how many bytes have already been written to correctly reference objects
/// inside the document.
pub struct DocWriter<W: io::Write> {
    w: W,
    len: usize,
    xref: HashMap<usize, usize>, // <object id, offset>
}

impl<W: io::Write> Writer<W> {
    /// Converts the current variant into a `Writer::Stream`.
    pub fn into_stream(self, id_seq: &mut IdSeq) -> Self {
        match self {
            Writer::Doc(w) => Writer::Stream(Stream::new(id_seq, w)),
            s => s,
        }
    }

    /// Ends the current `Stream<W>`, if it is currently one. Returns a reference to the ended
    /// stream.
    pub fn end_stream(&mut self) -> Result<Option<Reference<StreamRef>>, io::Error> {
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

impl<W: io::Write> DocWriter<W> {
    /// Constructs a new `DocWriter<W>` wrapping the given writer.
    pub fn new(w: W) -> Self {
        DocWriter {
            w,
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

    /// Writes the XREF table into into the wrapped writer of the `DocWriter<W>`.
    pub fn write_xref(&mut self) -> Result<(), io::Error> {
        writeln!(self.w, "xref")?;

        let mut from = 0;
        let mut to = 1;
        let mut offsets = Vec::with_capacity(self.xref.len());

        loop {
            if let Some(offset) = self.xref.remove(&to) {
                offsets.push(offset);
            } else {
                if from == 0 || !offsets.is_empty() {
                    writeln!(self.w, "{} {}", from, to - from)?;

                    if from == 0 {
                        writeln!(self.w, "0000000000 65535 f")?;
                    }

                    for offset in &offsets {
                        writeln!(self.w, "{:010} 00000 n", offset)?;
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

        w.write_xref().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../tests/fixtures/xref_1.txt"),
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

        w.write_xref().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../tests/fixtures/xref_2.txt"),
        );
    }
}
