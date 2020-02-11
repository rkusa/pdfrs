use std::io::{self, Write};

use crate::idseq::IdSeq;
use crate::writer::DocWriter;
use serde::Serialize;
use serde_pdf::{to_writer, Object, ObjectId, Reference};

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "")]
pub struct StreamMeta {
    pub length: Reference<usize>,
}

pub type StreamRef = ();

pub struct Stream<W: io::Write> {
    id: ObjectId,
    len_obj_id: ObjectId,
    len: usize,
    header_written: bool,
    wr: DocWriter<W>,
}

impl<W: io::Write> Stream<W> {
    pub fn new(id_seq: &mut IdSeq, wr: DocWriter<W>) -> Self {
        Stream {
            id: ObjectId::new(id_seq.next(), 0),
            len_obj_id: ObjectId::new(id_seq.next(), 0),
            len: 0,
            header_written: false,
            wr,
        }
    }

    pub fn to_reference(&self) -> Reference<StreamRef> {
        Reference::new(self.id.clone())
    }

    fn write_header(&mut self) -> Result<(), io::Error> {
        if !self.header_written {
            self.header_written = true;

            self.wr.add_xref(self.id.id());
            writeln!(self.wr, "{} {} obj", self.id.id(), self.id.rev())?;
            to_writer(
                &mut self.wr,
                &StreamMeta {
                    length: Reference::new(self.len_obj_id.clone()),
                },
            )
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
            writeln!(self.wr, "\nstream")?;
        }

        Ok(())
    }

    pub fn end(mut self) -> Result<DocWriter<W>, io::Error> {
        if self.len == 0 {
            return Ok(self.wr);
        }

        self.write_header()?;
        if self.len > 0 {
            writeln!(self.wr)?;
        }
        writeln!(self.wr, "endstream\nendobj\n")?;

        self.wr.add_xref(self.len_obj_id.id());
        let len_obj = Object::new(self.len_obj_id.id(), self.len_obj_id.rev(), self.len);
        to_writer(&mut self.wr, &len_obj)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        Ok(self.wr)
    }
}

impl<W> io::Write for Stream<W>
where
    W: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.write_header()?;

        let n = self.wr.write(buf)?;
        self.len += n;
        Ok(n)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.wr.flush()
    }
}
