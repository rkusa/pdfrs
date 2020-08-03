use std::collections::HashMap;
use std::io;

use crate::{ser::NAME_STREAM, Value};
use serde::{ser::SerializeTupleStruct, Serialize, Serializer};
use serde_bytes::Bytes;

pub struct Stream<D>
where
    D: Serialize,
{
    pub dict: D,
    pub data: Vec<u8>,
}

impl<D> Stream<D>
where
    D: Serialize,
{
    pub fn new(dict: D) -> Self {
        Stream {
            dict,
            data: Vec::new(),
        }
    }
}

impl<D> Serialize for Stream<D>
where
    D: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer
            .serialize_tuple_struct(NAME_STREAM, if self.data.is_empty() { 1 } else { 2 })?;
        s.serialize_field(&self.dict)?;
        if !self.data.is_empty() {
            s.serialize_field(Bytes::new(&self.data))?;
        }
        s.end()
    }
}

impl<D> io::Write for Stream<D>
where
    D: Serialize,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        (&mut self.data).write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        (&mut self.data).flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ser::to_string;

    #[test]
    fn test_serialize() {
        let mut dict: HashMap<String, Value> = HashMap::new();
        dict.insert("foo".to_string(), Value::String("bar".to_string()));

        let obj = Stream {
            dict,
            data: vec![b'a', b'b'],
        };
        assert_eq!(
            to_string(&obj).unwrap(),
            "<< /foo /bar >>\nstream\nab\nendstream\n"
        );
    }

    #[test]
    fn test_serialize_dict_only() {
        let mut dict: HashMap<String, Value> = HashMap::new();
        dict.insert("foo".to_string(), Value::String("bar".to_string()));

        let obj = Stream { dict, data: vec![] };
        assert_eq!(to_string(&obj).unwrap(), "<< /foo /bar >>\n");
    }
}
