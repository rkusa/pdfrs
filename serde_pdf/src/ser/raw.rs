use std::io;

use super::Serializer;
use crate::error::Error;
use chrono::prelude::*;
use serde::ser;

pub(crate) struct RawEmitter<'a, W>(pub(crate) &'a mut Serializer<W>)
where
    W: io::Write;

impl<'a, W> ser::Serializer for RawEmitter<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;
    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, _v: bool) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_i8(self, _v: i8) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_i16(self, _v: i16) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_i32(self, _v: i32) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_i64(self, _v: i64) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_u8(self, _v: u8) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_u16(self, _v: u16) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_u32(self, _v: u32) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_u64(self, _v: u64) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_f32(self, _v: f32) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_f64(self, _v: f64) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_char(self, _v: char) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_str(self, value: &str) -> Result<(), Self::Error> {
        self.0.output.write_all(value.as_bytes())?;
        Ok(())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_none(self) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize,
    {
        Err(Error::ExpectedString)
    }

    fn serialize_unit(self) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<(), Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error>
    where
        T: ser::Serialize,
    {
        Err(Error::ExpectedString)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error>
    where
        T: ser::Serialize,
    {
        Err(Error::ExpectedString)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::ExpectedString)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::ExpectedString)
    }
}

#[test]
fn datetime_serialization() {
    use chrono::FixedOffset;

    #[derive(Serialize)]
    struct Test {
        #[serde(with = "crate::datetime")]
        datetime: DateTime<FixedOffset>,
    }

    let test = Test {
        datetime: FixedOffset::east(1 * 3600)
            .ymd(2015, 2, 19)
            .and_hms(22, 33, 26),
    };

    assert_eq!(
        crate::ser::to_string(&test).unwrap(),
        "<<\n\t/Type /Test\n\t/datetime (D:20150219223326+01'00')\n>>"
    );
}
