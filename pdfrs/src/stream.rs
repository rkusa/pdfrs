use std::io::{self, Write};

use crate::idseq::IdSeq;
use crate::writer::DocWriter;
use serde::Serialize;
use serde_pdf::{to_writer, Object, ObjectId, Reference};

/// A type used to handle writing a PDF stream to a PDF document. It handles creating a
/// corresponding PDF object, keeping track of the stream's length as well as writing the stream
/// it all it's related meta data to the PDF document.
pub struct Stream<W: io::Write> {
    id: ObjectId,
    len_obj_id: ObjectId,
    len: usize,
    header_written: bool,
    wr: DocWriter<W>,
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

impl<W: io::Write> Stream<W> {
    /// Constructs a new PDF stream.
    pub fn new(id_seq: &mut IdSeq, wr: DocWriter<W>) -> Self {
        Stream {
            id: ObjectId::new(id_seq.next(), 0),
            len_obj_id: ObjectId::new(id_seq.next(), 0),
            len: 0,
            header_written: false,
            wr,
        }
    }

    /// Returns a PDF reference to the stream's PDF object.
    pub fn to_reference(&self) -> Reference<StreamRef> {
        Reference::new(self.id.clone())
    }

    /// Writes the stream's and its corresponding object's start markers, as well as writing its
    /// object properties (which includes a reference to its length object).
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

    /// Ends the PDF stream, which involves writing the stream's and corresponding object's end
    /// markers and the stream's length object.
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

    /// Begins a text object (BT - PDF spec 1.7 page 405).
    pub fn begin_text(&mut self) -> Result<(), io::Error> {
        // FIXME: move text operations into an object returned here to prevent nested BT.
        writeln!(self, "BT")
    }

    /// Ends a text object (ET - PDF spec 1.7 page 405).
    pub fn end_text(&mut self) -> Result<(), io::Error> {
        writeln!(self, "ET")
    }

    /// Sets the text matrix (Tm - PDF spec 1.7 page 406).
    #[allow(clippy::many_single_char_names)]
    pub fn set_text_matrix(
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
    }

    /// Sets the text leading (TL - PDF spec 1.7 page 398).
    pub fn set_text_leading(&mut self, leading: f64) -> Result<(), io::Error> {
        writeln!(self, "{:.3} TL", leading)
    }

    /// Sets the text font and font size (Tf - PDF spec 1.7 page 398).
    pub fn set_text_font(&mut self, font_id: usize, size: f64) -> Result<(), io::Error> {
        writeln!(self, "/F{} {:.3} Tf", font_id, size)
    }

    // Sets the color to use for non-stroking operations (sc - PDF spec 1.7 page 287).
    pub fn set_fill_color(&mut self, c1: f64, c2: f64, c3: f64) -> Result<(), io::Error> {
        writeln!(self, "{:.3} {:.3} {:.3} sc", c1, c2, c3)
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
