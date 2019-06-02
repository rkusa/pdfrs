use serde::ser::{self, Impossible, Serialize};
use std::borrow::Cow;
use std::io;

pub mod datetime;
mod raw;

use crate::error::{Error, Result};
use raw::RawEmitter;

pub(crate) const NAME_STREAM: &'static str = "$__pdf_stream";
pub(crate) const NAME_OBJECT: &'static str = "$__pdf_object";
pub(crate) const NAME_REFERENCE: &'static str = "$__pdf_reference";
pub(crate) const NAME_RAW: &'static str = "$__pdf_raw";

pub struct Serializer<W>
where
    W: io::Write,
{
    output: W,
    depth: usize,
}

pub fn to_string<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        output: Vec::new(),
        depth: 0,
    };
    value.serialize(&mut serializer)?;
    Ok(String::from_utf8_lossy(&serializer.output).to_string())
}

pub fn to_writer<W, T>(w: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: Serialize,
{
    let mut serializer = Serializer {
        output: w,
        depth: 0,
    };
    value.serialize(&mut serializer)?;
    Ok(())
}

struct MapKeySerializer<'a, W>
where
    W: io::Write,
{
    ser: &'a mut Serializer<W>,
}

pub struct Compound<'a, W>
where
    W: io::Write,
{
    ser: &'a mut Serializer<W>,
    is_first: bool,
}

pub enum TupleStruct<'a, W>
where
    W: io::Write,
{
    Compound(Compound<'a, W>),
    Stream {
        ser: &'a mut Serializer<W>,
        is_first: bool,
    },
    Object {
        ser: &'a mut Serializer<W>,
        ix: usize,
    },
    Reference {
        ser: &'a mut Serializer<W>,
        is_first: bool,
    },
}

impl<W> Serializer<W>
where
    W: io::Write,
{
    fn serialize_name(&mut self, name: &str) -> Result<()> {
        write!(self.output, "/")?;
        let mut from = 0;
        // Note: iterating over bytes will break utf8 characters
        for (i, ch) in name.bytes().enumerate() {
            match ch {
                0x00 => return Err(Error::Eof), // TODO: other error?
                // characters that need to be escaped (outside of ! to ~ and delimiter characters)
                0x01...0x20
                | 0x7F...0xFF
                | b'('
                | b')'
                | b'<'
                | b'>'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'/'
                | b'%'
                | b'#' => {
                    if i > from {
                        self.output
                            .write_all(name.get(from..i).unwrap().as_bytes())?;
                    }
                    write!(self.output, "#{:x}", ch)?;

                    from = i + 1;
                }
                0x21...0x7E => continue,
                _ => {
                    // eprintln!("FAIL {} {:?}", ch, ch.to_digit(10));
                    return Err(Error::ExpectedBoolean);
                } // _ => return Err(Error::ExpectedBoolean), // TODO: other error
            }
        }
        if let Some(remaining) = name.get(from..) {
            self.output.write_all(remaining.as_bytes())?;
        }
        Ok(())
    }
}

impl<'a, W> ser::Serializer for &'a mut Serializer<W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Compound<'a, W>;
    type SerializeTuple = Compound<'a, W>;
    type SerializeTupleStruct = TupleStruct<'a, W>;
    type SerializeTupleVariant = Compound<'a, W>;
    type SerializeMap = Compound<'a, W>;
    type SerializeStruct = Compound<'a, W>;
    type SerializeStructVariant = Compound<'a, W>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        write!(self.output, "{}", if v { "true" } else { "false" })?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.output.write_all(v.to_string().as_bytes())?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.output.write_all(v.to_string().as_bytes())?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.output.write_all(v.to_string().as_bytes())?;
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        write!(self.output, "({})", v)?;
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_name(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        write!(self.output, "stream\n")?;
        self.output.write_all(v)?;
        write!(self.output, "\nendstream")?;
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        write!(self.output, "null")?;
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_name(variant)
    }

    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match name {
            NAME_RAW => value.serialize(RawEmitter(self)),
            _ => value.serialize(self),
        }
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "<< ")?;
        self.serialize_name(variant)?;
        write!(self.output, " ")?;
        value.serialize(&mut *self)?;
        write!(self.output, " >>")?;
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        write!(self.output, "[")?;
        Ok(Compound {
            ser: self,
            is_first: true,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        match name {
            NAME_STREAM => Ok(TupleStruct::Stream {
                ser: self,
                is_first: true,
            }),
            NAME_OBJECT => Ok(TupleStruct::Object { ser: self, ix: 0 }),
            NAME_REFERENCE => Ok(TupleStruct::Reference {
                ser: self,
                is_first: true,
            }),
            _ => Ok(TupleStruct::Compound(self.serialize_seq(Some(len))?)),
        }
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        write!(self.output, "<< ")?;
        self.serialize_name(variant)?;
        write!(self.output, " [")?;
        Ok(Compound {
            ser: self,
            is_first: true,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        write!(self.output, "<<")?;
        Ok(Compound {
            ser: self,
            is_first: true,
        })
    }

    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        write!(self.output, "<<\n")?;

        self.depth += 1;

        if name != "" {
            write!(self.output, "{}", "\t".repeat(self.depth))?;
            self.serialize_name("Type")?;
            write!(self.output, " ")?;
            self.serialize_name(name)?;
            write!(self.output, "\n")?;
        }

        Ok(Compound {
            ser: self,
            is_first: true,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        write!(self.output, "<< ")?;
        self.serialize_name(variant)?;
        write!(self.output, " << ")?;
        Ok(Compound {
            ser: self,
            is_first: true,
        })
    }
}

impl<'a, W> ser::SerializeSeq for Compound<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.is_first {
            self.is_first = false;
        } else {
            write!(self.ser.output, " ")?;
        }
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        write!(self.ser.output, "]")?;
        Ok(())
    }
}

impl<'a, W> ser::SerializeTuple for Compound<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.is_first {
            self.is_first = false;
        } else {
            write!(self.ser.output, " ")?;
        }
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        write!(self.ser.output, "]")?;
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleStruct for TupleStruct<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        use serde::ser::SerializeTuple;

        match self {
            TupleStruct::Compound(s) => s.serialize_element(value),
            TupleStruct::Stream {
                ser,
                ref mut is_first,
            } => {
                if *is_first {
                    *is_first = false;
                } else {
                    write!(ser.output, "\n")?;
                }
                value.serialize(&mut **ser)
            }
            TupleStruct::Object { ser, ref mut ix } => {
                match *ix {
                    1 => write!(ser.output, " ")?,
                    2 => write!(ser.output, " obj\n")?,
                    _ => {}
                }
                *ix += 1;
                value.serialize(&mut **ser)
            }
            TupleStruct::Reference {
                ser,
                ref mut is_first,
            } => {
                if *is_first {
                    *is_first = false;
                } else {
                    write!(ser.output, " ")?;
                }
                value.serialize(&mut **ser)
            }
        }
    }

    fn end(self) -> Result<()> {
        use serde::ser::SerializeTuple;

        match self {
            TupleStruct::Compound(s) => s.end(),
            TupleStruct::Stream { ser, .. } => {
                write!(ser.output, "\n")?;
                Ok(())
            }
            TupleStruct::Object { ser, .. } => {
                write!(ser.output, "\nendobj\n\n")?;
                Ok(())
            }
            TupleStruct::Reference { ser, .. } => {
                write!(ser.output, " R")?;
                Ok(())
            }
        }
    }
}

impl<'a, W> ser::SerializeTupleVariant for Compound<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.is_first {
            self.is_first = false;
        } else {
            write!(self.ser.output, " ")?;
        }
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        write!(self.ser.output, "] >>")?;
        Ok(())
    }
}

impl<'a, W> ser::SerializeStruct for Compound<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        write!(self.ser.output, "{}", "\t".repeat(self.ser.depth))?;

        self.ser.serialize_name(key)?;
        write!(self.ser.output, " ")?;
        value.serialize(&mut *self.ser)?;

        write!(self.ser.output, "\n")?;

        Ok(())
    }

    fn end(self) -> Result<()> {
        self.ser.depth -= 1;
        write!(self.ser.output, "{}>>", "\t".repeat(self.ser.depth))?;
        Ok(())
    }
}

impl<'a, W> ser::SerializeStructVariant for Compound<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ser.serialize_name(key)?;
        write!(self.ser.output, " ")?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        write!(self.ser.output, " >> >>")?;
        Ok(())
    }
}

impl<'a, W> ser::SerializeMap for Compound<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        write!(self.ser.output, " ")?;
        key.serialize(MapKeySerializer { ser: self.ser })
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        write!(self.ser.output, " ")?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        write!(self.ser.output, " >>")?;
        Ok(())
    }
}

impl<'a, W> ser::Serializer for MapKeySerializer<'a, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        self.ser.serialize_name(value)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.ser.serialize_str(variant)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_bool(self, _value: bool) -> Result<()> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_i8(self, value: i8) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_i16(self, value: i16) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_i32(self, value: i32) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_i64(self, value: i64) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_u8(self, value: u8) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_u16(self, value: u16) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_u32(self, value: u32) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_u64(self, value: u64) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_f32(self, value: f32) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_f64(self, value: f64) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_char(self, value: char) -> Result<()> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<()> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_unit(self) -> Result<()> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: Serialize,
    {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_none(self) -> Result<()> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<()>
    where
        T: Serialize,
    {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::MapKeyMustBeAString)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::MapKeyMustBeAString)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    use super::{to_string, Serializer};

    #[test]
    fn test_bool() {
        assert_eq!(to_string(&true).unwrap(), "true");
        assert_eq!(to_string(&false).unwrap(), "false");
    }

    #[test]
    fn test_null() {
        let val: Option<()> = None;
        assert_eq!(to_string(&val).unwrap(), "null");
    }

    #[test]
    fn test_option() {
        assert_eq!(to_string(&Some(42)).unwrap(), "42");
    }

    #[test]
    fn test_numbers() {
        assert_eq!(to_string(&42u8).unwrap(), "42");
        assert_eq!(to_string(&42i8).unwrap(), "42");
        assert_eq!(to_string(&42u16).unwrap(), "42");
        assert_eq!(to_string(&42i16).unwrap(), "42");
        assert_eq!(to_string(&42u32).unwrap(), "42");
        assert_eq!(to_string(&42i32).unwrap(), "42");
        assert_eq!(to_string(&42u64).unwrap(), "42");
        assert_eq!(to_string(&42i64).unwrap(), "42");
    }

    #[test]
    fn test_string() {
        let s = String::from(r#"0ab(\fo)?!€"#);
        assert_eq!(to_string(&s).unwrap(), r#"/0ab#28\fo#29?!#e2#82#ac"#);
    }

    #[test]
    fn test_seq() {
        assert_eq!(to_string(&vec![1, 2, 3, 4]).unwrap(), "[1 2 3 4]");
        assert_eq!(to_string(&vec!["a", "b"]).unwrap(), "[/a /b]");
    }

    #[test]
    fn test_char() {
        assert_eq!(to_string(&'a').unwrap(), "(a)");
    }

    #[test]
    fn test_tuple() {
        assert_eq!(to_string(&(3, 2, 1)).unwrap(), "[3 2 1]");
    }

    #[test]
    fn test_name() {
        fn serialize(name: &str) -> String {
            let mut ser = Serializer {
                output: Vec::new(),
                depth: 0,
            };
            ser.serialize_name(name).unwrap();
            String::from_utf8_lossy(&ser.output).to_string()
        }

        assert_eq!(&serialize("Name1"), "/Name1");
        assert_eq!(
            &serialize("A;Name_With-Various***Characters?"),
            "/A;Name_With-Various***Characters?"
        );
        assert_eq!(&serialize("1.2"), "/1.2");
        assert_eq!(&serialize("$$"), "/$$");
        assert_eq!(&serialize("@pattern"), "/@pattern");
        assert_eq!(&serialize(".notdef"), "/.notdef");

        assert_eq!(&serialize("Adobe Green"), "/Adobe#20Green");
        assert_eq!(&serialize("PANTONE 5757 CV"), "/PANTONE#205757#20CV");
        assert_eq!(
            &serialize("paired()parentheses"),
            "/paired#28#29parentheses"
        );
        assert_eq!(&serialize("The_Key_of_F#_Minor"), "/The_Key_of_F#23_Minor");
    }

    #[test]
    fn test_struct() {
        #[derive(Serialize)]
        struct Test {
            int: u32,
            seq: Vec<&'static str>,
        }

        let test = Test {
            int: 1,
            seq: vec!["a", "b"],
        };
        let expected = "<<\n\t/Type /Test\n\t/int 1\n\t/seq [/a /b]\n>>";
        assert_eq!(to_string(&test).unwrap(), expected);
    }

    #[test]
    fn test_struct_nested() {
        #[derive(Serialize)]
        struct Inner {
            int: u32,
        }

        #[derive(Serialize)]
        struct Test {
            int: u32,
            inner: Inner,
        }

        let test = Test {
            int: 1,
            inner: Inner { int: 2 },
        };
        let expected =
            "<<\n\t/Type /Test\n\t/int 1\n\t/inner <<\n\t\t/Type /Inner\n\t\t/int 2\n\t>>\n>>";
        assert_eq!(to_string(&test).unwrap(), expected);
    }

    #[test]
    fn test_newtype_struct() {
        #[derive(Serialize)]
        struct Test(i32);

        let test = Test(1);
        let expected = r#"1"#;
        assert_eq!(to_string(&test).unwrap(), expected);
    }

    #[test]
    fn test_tuple_struct() {
        #[derive(Serialize)]
        struct Test(i32, i32);

        let test = Test(1, 2);
        let expected = r#"[1 2]"#;
        assert_eq!(to_string(&test).unwrap(), expected);
    }

    #[test]
    fn test_map() {
        use std::collections::HashMap;

        let mut data = HashMap::new();
        data.insert("foo", "bar");

        let expected = r#"<< /foo /bar >>"#;
        assert_eq!(to_string(&data).unwrap(), expected);
    }

    #[test]
    fn test_enum() {
        #[derive(Serialize)]
        enum E {
            Unit,
            Newtype(u32),
            Tuple(u32, u32),
            Struct { a: u32 },
        }

        let u = E::Unit;
        let expected = "/Unit";
        assert_eq!(to_string(&u).unwrap(), expected);

        let n = E::Newtype(1);
        let expected = "<< /Newtype 1 >>";
        assert_eq!(to_string(&n).unwrap(), expected);

        let t = E::Tuple(1, 2);
        let expected = "<< /Tuple [1 2] >>";
        assert_eq!(to_string(&t).unwrap(), expected);

        let s = E::Struct { a: 1 };
        let expected = "<< /Struct << /a 1 >> >>";
        assert_eq!(to_string(&s).unwrap(), expected);
    }

    #[test]
    fn test_bytes() {
        use serde_bytes::Bytes;
        #[derive(Serialize)]
        struct Test {
            #[serde(with = "serde_bytes")]
            data: Vec<u8>,
        }

        let test = Bytes::new("foobar".as_bytes());
        let expected = "stream\nfoobar\nendstream";
        assert_eq!(to_string(&test).unwrap(), expected);
    }
}
