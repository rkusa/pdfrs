use std::convert::AsRef;
use std::io::{self, Cursor, Read};

pub struct LimitRead<T: io::Read> {
    inner: T,
    limit: usize,
    already_read: usize,
}

impl<T> LimitRead<T>
where
    T: io::Read,
{
    #[allow(unused)]
    pub fn new(inner: T, limit: usize) -> Self {
        Self {
            inner,
            limit,
            already_read: 0,
        }
    }

    pub fn discard(mut self) -> Result<(), io::Error> {
        let n = self.limit - self.already_read;
        if n == 0 {
            return Ok(());
        }

        let mut buf = vec![0; n];
        self.read_exact(&mut buf[..])?;
        Ok(())
    }
}

impl<'a> LimitRead<&'a [u8]> {
    pub fn from_cursor<C>(inner: &'a Cursor<C>, limit: usize) -> Self
    where
        C: AsRef<[u8]> + 'a,
    {
        let pos = inner.position() as usize;
        Self {
            inner: &inner.get_ref().as_ref()[pos..],
            limit,
            already_read: 0,
        }
    }
}

impl<T> io::Read for LimitRead<T>
where
    T: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.already_read == self.limit {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        let cap = buf.len().min(self.limit - self.already_read);
        let n = self.inner.read(&mut buf[..cap])?;
        self.already_read += n;
        Ok(n)
    }
}

impl<T> AsRef<[u8]> for LimitRead<T>
where
    T: io::Read + AsRef<[u8]>,
{
    fn as_ref(&self) -> &[u8] {
        &self.inner.as_ref()[..self.limit]
    }
}

#[cfg(test)]
mod test {
    use std::io::{Cursor, Read};

    use super::*;

    #[test]
    fn test_limit_read() {
        let data = b"foobar".to_vec();
        let mut rd = LimitRead::new(Cursor::new(data), 5);

        let mut buf = [0; 2];
        assert_eq!((rd.read(&mut buf).unwrap(), &buf), (2, b"fo"));
        assert_eq!((rd.read(&mut buf).unwrap(), &buf), (2, b"ob"));
        assert_eq!((rd.read(&mut buf).unwrap(), &buf[..1]), (1, &b"a"[..]));
        assert_eq!(
            rd.read(&mut buf).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
