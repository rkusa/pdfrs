use std::io;

pub struct LimitRead<T: io::Read> {
    inner: T,
    limit: usize,
    already_read: usize,
}

impl<T> LimitRead<T>
where
    T: io::Read,
{
    pub fn new(inner: T, limit: usize) -> Self {
        Self {
            inner,
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
            return Ok(0);
        }

        let cap = buf.len().min(self.limit - self.already_read);
        let n = self.inner.read(&mut buf[..cap])?;
        self.already_read += n;
        Ok(n)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Cursor, Read};

    use super::*;

    #[test]
    fn limit_read() {
        let data = "foobar".as_bytes().to_vec();
        let mut rd = LimitRead::new(Cursor::new(data), 5);

        let mut buf = [0; 2];
        assert_eq!((rd.read(&mut buf).unwrap(), &buf), (2, b"fo"));
        assert_eq!((rd.read(&mut buf).unwrap(), &buf), (2, b"ob"));
        assert_eq!((rd.read(&mut buf).unwrap(), &buf[..1]), (1, &b"a"[..]));
        assert_eq!(rd.read(&mut buf).unwrap(), 0);
    }
}
