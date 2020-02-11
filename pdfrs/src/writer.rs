use std::collections::HashMap;
use std::{io, mem};

use crate::idseq::IdSeq;
use crate::stream::{Stream, StreamRef};
use serde_pdf::Reference;

pub enum Writer<W: io::Write> {
    Doc(DocWriter<W>),
    Stream(Stream<W>),
    Null,
}

pub struct DocWriter<W: io::Write> {
    w: W,
    len: usize,
    xref: HashMap<usize, usize>, // <object id, offset>
}

impl<W: io::Write> DocWriter<W> {
    pub fn new(w: W) -> Self {
        DocWriter {
            w,
            len: 0,
            xref: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn add_xref(&mut self, id: usize) {
        self.xref.insert(id, self.len);
    }

    pub fn write_xref(&mut self) -> Result<(), io::Error> {
        write!(self.w, "xref\n")?;

        let mut from = 0;
        let mut to = 1;
        let mut offsets = Vec::with_capacity(self.xref.len());

        loop {
            if let Some(offset) = self.xref.remove(&to) {
                offsets.push(offset);
            } else {
                if from == 0 || !offsets.is_empty() {
                    write!(self.w, "{} {}\n", from, to - from)?;

                    if from == 0 {
                        write!(self.w, "0000000000 65535 f\n")?;
                    }

                    for offset in &offsets {
                        write!(self.w, "{:010} 00000 n\n", offset)?;
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
    pub fn into_stream(self, id_seq: &mut IdSeq) -> Self {
        match self {
            Writer::Doc(w) => Writer::Stream(Stream::new(id_seq, w)),
            s => s,
        }
    }

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

        w.write_xref().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&b).to_string(),
            include_str!("../test/results/xref_2.txt"),
        );
    }
}
