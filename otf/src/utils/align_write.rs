use std::io::{self, Write};

pub struct AlignWrite<W: Write> {
    inner: W,
    align: usize,
    written: usize,
}

impl<W> AlignWrite<W>
where
    W: Write,
{
    pub fn new(inner: W, align: usize) -> Self {
        AlignWrite {
            inner,
            align,
            written: 0,
        }
    }

    pub fn end_aligned(mut self) -> Result<usize, io::Error> {
        if self.written % self.align != 0 {
            let buf = vec![0; self.align - (self.written % self.align)];
            self.write_all(&buf)?;
        }

        Ok(self.written)
    }
}

impl<W> Write for AlignWrite<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let n = self.inner.write(buf)?;
        self.written += n;
        Ok(n)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.inner.flush()
    }
}
