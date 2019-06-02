use std::io::{self, Write};

use crate::document::{DocWriter, IdSeq};
use serde::Serialize;
use serde_pdf::{to_writer, Object, ObjectId, Reference};

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[serde(rename = "")]
pub struct StreamMeta {
    pub length: Reference<usize>,
}

pub type StreamRef = ();

pub struct Stream<'a, W: io::Write> {
    id: ObjectId,
    len_obj_id: ObjectId,
    len: usize,
    header_written: bool,
    wr: &'a mut DocWriter<W>,
}

impl<'a, W: io::Write> Stream<'a, W> {
    pub fn new(id_seq: &mut IdSeq, wr: &'a mut DocWriter<W>) -> Self {
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
            write!(self.wr, "{} {} obj\n", self.id.id(), self.id.rev())?;
            to_writer(
                &mut self.wr,
                &StreamMeta {
                    length: Reference::new(self.len_obj_id.clone()),
                },
            )
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
            write!(self.wr, "\nstream\n")?;
        }

        Ok(())
    }

    pub fn end(mut self) -> Result<(), io::Error> {
        if self.len == 0 {
            return Ok(());
        }

        self.write_header()?;
        if self.len > 0 {
            write!(self.wr, "\n")?;
        }
        write!(self.wr, "endstream\nendobj\n\n")?;

        self.wr.add_xref(self.len_obj_id.id());
        let len_obj = Object::new(self.len_obj_id.id(), self.len_obj_id.rev(), self.len);
        to_writer(&mut self.wr, &len_obj)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        Ok(())
    }
}

impl<'a, W> io::Write for Stream<'a, W>
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
