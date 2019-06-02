use std::convert::TryFrom;
use std::io::prelude::*;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg};

use num_traits::float::Float;
use serde::de::{
    self, Deserialize, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

use crate::error::{Error, Result};

pub struct Deserializer<R> {
    input: R,
    peek: Option<u8>,
}

impl<R> Deserializer<R>
where
    R: Read,
{
    pub fn new(input: R) -> Self {
        Deserializer { input, peek: None }
    }
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut input = s.as_bytes();
    let mut deserializer = Deserializer::new(input);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.is_empty() {
        Ok(t)
    } else {
        Err(Error::TrailingCharacters)
    }
}

impl<R> Deserializer<R>
where
    R: Read,
{
    /// Look at the first character in the input without consuming it.
    fn peek_char(&mut self) -> Result<Option<u8>> {
        if self.peek.is_some() {
            return Ok(self.peek);
        }

        let next = self.next_char()?;
        self.peek = next;
        Ok(next)
    }

    /// Consume the first character in the input.
    fn next_char(&mut self) -> Result<Option<u8>> {
        if let Some(ch) = self.peek.take() {
            Ok(Some(ch))
        } else {
            let mut b = [0; 1];
            let n = self.input.read(&mut b)?;
            if n == 1 {
                Ok(Some(b[0]))
            } else {
                Ok(None)
            }
        }
    }

    /// Discard the char from a previous look ahead.
    fn discard_char(&mut self) {
        self.peek = None;
    }

    /// Discard any whitespace (as defined by PDF Spec 1.7)
    fn discard_whitespace(&mut self) -> Result<bool> {
        let mut has_whitespace = false;
        loop {
            match self.peek_char()? {
                Some(0x00) | Some(0x09) | Some(0x0A) | Some(0x0C) | Some(0x0D) | Some(0x20) => {
                    self.discard_char();
                    has_whitespace = true;
                }
                _ => return Ok(has_whitespace),
            }
        }
    }

    fn parse_ident(&mut self, ident: &[u8]) -> Result<bool> {
        for expected in ident {
            if let Some(ch) = self.next_char()? {
                if ch != *expected {
                    return Ok(false);
                }
            } else {
                return Err(Error::Eof);
            }
        }

        Ok(true)
    }

    fn parse_bool(&mut self) -> Result<bool> {
        match self.peek_char()? {
            Some(b't') => {
                self.discard_char();
                if !self.parse_ident(b"rue")? {
                    return Err(Error::ExpectedBoolean);
                }
                Ok(true)
            }
            Some(b'f') => {
                self.discard_char();
                if !self.parse_ident(b"alse")? {
                    return Err(Error::ExpectedBoolean);
                }
                Ok(false)
            }
            _ => Err(Error::ExpectedBoolean),
        }
    }

    fn parse_null(&mut self) -> Result<()> {
        if self.peek_char()? == Some(b'n') {
            self.discard_char();
            if self.parse_ident(b"ull")? {
                return Ok(());
            }
        }
        Err(Error::ExpectedNull)
    }

    fn parse_unsigned<T>(&mut self, allow_decimal: bool) -> Result<T>
    where
        T: CheckedMul + CheckedAdd + TryFrom<u8>,
    {
        if self.peek_char()? == Some(b'+') {
            self.discard_char();
        }

        if self.peek_char()? == Some(b'-') {
            return Err(Error::ExpectedUnsignedInteger);
        }

        let ten = T::try_from(10).or(Err(Error::ExpectedInteger))?;
        let mut val = T::try_from(0).or(Err(Error::ExpectedInteger))?;
        loop {
            match self.peek_char()? {
                Some(ch @ b'0'...b'9') => {
                    val = match val.checked_mul(&ten) {
                        Some(v) => v,
                        None => return Err(Error::NumberOverflow),
                    };
                    val = match val
                        .checked_add(&T::try_from(ch - b'0').or(Err(Error::ExpectedInteger))?)
                    {
                        Some(v) => v,
                        None => return Err(Error::NumberOverflow),
                    };
                    self.discard_char();
                }
                Some(b'.') => {
                    if allow_decimal {
                        return Ok(val);
                    } else {
                        return Err(Error::ExpectedFloat);
                    }
                }
                Some(ch) if (ch as char).is_whitespace() => {
                    return Ok(val);
                }
                Some(b']') | None => {
                    return Ok(val);
                }
                ch => {
                    return Err(Error::ExpectedInteger);
                }
            }
        }
    }

    fn parse_signed<T>(&mut self, allow_decimal: bool) -> Result<T>
    where
        T: Neg<Output = T> + CheckedMul + CheckedAdd + TryFrom<u8>,
    {
        match self.peek_char()? {
            Some(b'-') => {
                self.discard_char();
                let val: T = self.parse_unsigned(allow_decimal)?;
                Ok(val.neg())
            }
            Some(b'0'...b'9') | Some(b'+') | Some(b'.') => {
                let val: T = self.parse_unsigned(allow_decimal)?;
                Ok(val)
            }
            _ => Err(Error::ExpectedInteger),
        }
    }

    fn parse_float<T>(&mut self) -> Result<T>
    where
        T: Neg<Output = T> + CheckedMul + CheckedAdd + Float + TryFrom<u8>, // CheckedAdd
    {
        let mut val: T = self.parse_signed(true)?;

        if self.peek_char()? == Some(b'.') {
            self.discard_char();

            let mut ten = T::try_from(10).or(Err(Error::ExpectedInteger))?;
            let mut p: i32 = 0;
            loop {
                match self.peek_char()? {
                    Some(ch @ b'0'...b'9') => {
                        self.discard_char();

                        p -= 1;
                        let mut d = T::try_from(ch - b'0').or(Err(Error::ExpectedInteger))?;
                        d = match d.checked_mul(&ten.powi(p)) {
                            Some(v) => v,
                            None => return Err(Error::NumberOverflow),
                        };

                        if val.is_sign_negative() {
                            d = d.neg();
                        }

                        val = match val.checked_add(&d) {
                            Some(v) => v,
                            None => return Err(Error::NumberOverflow),
                        };
                    }
                    Some(ch) if (ch as char).is_whitespace() => {
                        return Ok(val);
                    }
                    None => {
                        return Ok(val);
                    }
                    _ => {
                        return Err(Error::ExpectedInteger);
                    }
                }
            }
        }

        Ok(val)
    }

    fn parse_string(&mut self) -> Result<String> {
        fn win1252_to_unicode(code: u32) -> Result<char> {
            let ch = match code {
                128 => '\u{20ac}',
                130 => '\u{201a}',
                131 => '\u{0192}',
                132 => '\u{201e}',
                133 => '\u{2026}',
                134 => '\u{2020}',
                135 => '\u{2021}',
                136 => '\u{02c6}',
                137 => '\u{2030}',
                138 => '\u{0160}',
                139 => '\u{2039}',
                140 => '\u{0152}',
                142 => '\u{017d}',
                145 => '\u{2018}',
                146 => '\u{2019}',
                147 => '\u{201c}',
                148 => '\u{201d}',
                149 => '\u{2022}',
                150 => '\u{2013}',
                151 => '\u{2014}',
                152 => '\u{02dc}',
                153 => '\u{2122}',
                154 => '\u{0161}',
                155 => '\u{203a}',
                156 => '\u{0153}',
                158 => '\u{017e}',
                159 => '\u{0178}',
                _ => match std::char::from_u32(code) {
                    Some(ch) => ch,
                    None => return Err(Error::InvalidEscapeSequence),
                },
            };
            Ok(ch)
        }

        if self.peek_char()? == Some(b'/') {
            return self.parse_name();
        }

        if self.peek_char()? != Some(b'(') {
            return Err(Error::ExpectedString);
        }

        self.discard_char();

        let mut chars: Vec<char> = Vec::new();
        let mut opened_parentheses = 0;
        while let Some(ch) = self.next_char()? {
            match ch {
                b'(' => {
                    opened_parentheses += 1;
                    chars.push(ch as char);
                }
                b')' => {
                    if opened_parentheses == 0 {
                        // we are done
                        return Ok(chars.into_iter().collect());
                    }

                    opened_parentheses -= 1;
                    chars.push(ch as char);
                }
                b'\\' => {
                    match self.next_char()? {
                        Some(b'n') => chars.push(b'\n' as char),
                        Some(b'r') => chars.push(b'\r' as char),
                        Some(b't') => chars.push(b'\t' as char),
                        Some(b'b') => chars.push(0x08 as char), // \b
                        Some(b'f') => chars.push(0x0C as char), // \f
                        Some(b'(') => chars.push(b'(' as char),
                        Some(b')') => chars.push(b')' as char),
                        Some(b'\\') => chars.push(b'\\' as char),
                        Some(b'\n') => {}
                        Some(c1 @ b'0'...b'9') => {
                            let mut bytes = Vec::with_capacity(3);
                            bytes.push(c1);

                            // we take up to three, ie, two more bytes
                            for _ in 1..=2 {
                                if let Some(c @ b'0'...b'9') = self.peek_char()? {
                                    bytes.push(self.next_char()?.unwrap());
                                }
                            }

                            let octal = std::str::from_utf8(&bytes).unwrap();
                            let code = match u32::from_str_radix(octal, 8) {
                                Ok(c) => c,
                                Err(_) => return Err(Error::InvalidEscapeSequence),
                            };
                            eprintln!("Code is {}", code);
                            chars.push(win1252_to_unicode(code)?);
                        }
                        _ => return Err(Error::InvalidEscapeSequence),
                    }
                }
                _ => chars.push(ch as char),
            }
        }

        Err(Error::Eof)
    }

    fn parse_name(&mut self) -> Result<String> {
        if self.peek_char()? != Some(b'/') {
            return Err(Error::ExpectedName);
        }

        self.discard_char();

        let mut name = String::new();
        while let Some(ch) = self.peek_char()? {
            match ch {
                // escape sequence
                b'#' => {
                    self.discard_char();
                    if let (Some(n1), Some(n2)) = (self.next_char()?, self.next_char()?) {
                        let bytes = &[n1, n2];
                        let code = std::str::from_utf8(bytes).unwrap();
                        let code = u8::from_str_radix(code, 16).unwrap();
                        name.push(code as char);
                    } else {
                        return Err(Error::InvalidEscapeSequence);
                    }
                }
                0x21...0x7E => {
                    self.discard_char();
                    name.push(ch as char);
                }
                _ => {
                    // other characters cannot occur inside a name, so we are done here
                    break;
                }
            }
        }

        Ok(name)
    }
}

impl<'de, 'a, R> de::Deserializer<'de> for &'a mut Deserializer<R>
where
    R: Read,
{
    type Error = Error;

    // Look at the input data to decide what Serde data model type to
    // deserialize as. Not all data formats are able to support this operation.
    // Formats that support `deserialize_any` are known as self-describing.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_char()? {
            Some(b'n') => self.deserialize_unit(visitor), // null
            Some(b't') | Some(b'f') => self.deserialize_bool(visitor),
            Some(b'(') | Some(b'/') => self.deserialize_str(visitor),
            Some(b'0'...b'9') | Some(b'+') => self.deserialize_u64(visitor),
            Some(b'-') => self.deserialize_i64(visitor),
            Some(b'.') => self.deserialize_f64(visitor),
            Some(b'[') => self.deserialize_seq(visitor),
            Some(b'<') => self.deserialize_map(visitor),
            // TODO:
            // - Reference (2 0 R)
            // - <...> str
            // - (...) other str
            // - << >> map
            // - 10 0 obj\n endobj
            // - stream endstream
            _ => Err(Error::Syntax),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse_signed(false)?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_signed(false)?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse_signed(false)?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse_signed(false)?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse_unsigned(false)?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse_unsigned(false)?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse_unsigned(false)?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_unsigned(false)?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(self.parse_float()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.parse_float()?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let s = self.parse_string()?;
        if s.len() != 1 {
            return Err(Error::ExpectedChar);
        }

        let ch = s.chars().next().ok_or(Error::ExpectedChar)?;
        visitor.visit_char(ch)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_char()? == Some(b'n') {
            self.parse_null()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.parse_null()?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
        //        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
        //        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Parse the opening bracket of the sequence.
        if self.peek_char()? == Some(b'[') {
            self.discard_char();

            // Give the visitor access to each element of the sequence.
            let value = visitor.visit_seq(SpaceSeparated::new(&mut self))?;

            // Parse the closing bracket of the sequence.
            if self.next_char()? == Some(b']') {
                Ok(value)
            } else {
                Err(Error::ExpectedArrayEnd)
            }
        } else {
            Err(Error::ExpectedArray)
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
        //        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
        //        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Parse the opening brace of the map.
        if (self.next_char()?, self.next_char()?) == (Some(b'<'), Some(b'<')) {
            self.discard_whitespace()?;
            eprintln!("Struct");

            // Give the visitor access to each entry of the map.
            let value = visitor.visit_map(SpaceSeparated::new(&mut self))?;
            // Parse the closing brace of the map.
            if (self.next_char()?, self.next_char()?) == (Some(b'>'), Some(b'>')) {
                Ok(value)
            } else {
                Err(Error::ExpectedMapEnd)
            }
        } else {
            Err(Error::ExpectedMap)
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_char()? == Some(b'/') {
            // Visit a unit variant.
            visitor.visit_enum(self.parse_string()?.into_deserializer())
        } else if (self.next_char()?, self.next_char()?) == (Some(b'<'), Some(b'<')) {
            self.discard_whitespace()?;

            // Visit a newtype variant, tuple variant, or struct variant.
            let value = visitor.visit_enum(Enum::new(self))?;

            self.discard_whitespace()?;

            dbg!(self.peek_char()?);

            // Parse the closing brace of the map.
            if (self.next_char()?, self.next_char()?) == (Some(b'>'), Some(b'>')) {
                Ok(value)
            } else {
                Err(Error::ExpectedMapEnd)
            }
        } else {
            Err(Error::ExpectedMap)
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        eprintln!("Identifier");
        visitor.visit_string(self.parse_name()?)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
        //        self.deserialize_any(visitor)
    }
}

struct SpaceSeparated<'a, R> {
    de: &'a mut Deserializer<R>,
    first: bool,
}

impl<'a, 'de, R> SpaceSeparated<'a, R>
where
    R: Read,
{
    fn new(de: &'a mut Deserializer<R>) -> Self {
        SpaceSeparated { de, first: true }
    }
}

impl<'de, 'a, R> SeqAccess<'de> for SpaceSeparated<'a, R>
where
    R: Read,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        // Check if there are no more elements.
        if self.de.peek_char()? == Some(b']') {
            return Ok(None);
        }

        // Space is required before every element except the first.
        if !self.first && self.de.next_char()? != Some(b' ') {
            return Err(Error::ExpectedArrayComma);
        }
        self.first = false;

        // Deserialize an array element.
        seed.deserialize(&mut *self.de).map(Some)
    }
}

// `MapAccess` is provided to the `Visitor` to give it the ability to iterate
// through entries of the map.
impl<'de, 'a, R> MapAccess<'de> for SpaceSeparated<'a, R>
where
    R: Read,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        let had_whitespace = self.de.discard_whitespace()?;

        // Check if there are no more entries.
        if self.de.peek_char()? == Some(b'>') {
            return Ok(None);
        }

        if !self.first && !had_whitespace {
            return Err(Error::ExpectedMapComma);
        }
        self.first = false;

        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        if !self.de.discard_whitespace()? {
            return Err(Error::ExpectedMapColon);
        }

        seed.deserialize(&mut *self.de)
    }
}

struct Enum<'a, R> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R> Enum<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        Enum { de }
    }
}

impl<'de, 'a, R> EnumAccess<'de> for Enum<'a, R>
where
    R: Read,
{
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let val = seed.deserialize(&mut *self.de)?;

        if !self.de.discard_whitespace()? {
            return Err(Error::ExpectedMapColon);
        }

        Ok((val, self))
    }
}

// `VariantAccess` is provided to the `Visitor` to give it the ability to see
// the content of the single variant that it decided to deserialize.
impl<'de, 'a, R> VariantAccess<'de> for Enum<'a, R>
where
    R: Read,
{
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::ExpectedString)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}

pub trait CheckedAdd: Sized + Add<Self, Output = Self> {
    /// Adds two numbers, checking for overflow. If overflow happens, `None` is
    /// returned.
    fn checked_add(&self, v: &Self) -> Option<Self>;
}

macro_rules! checked_impl {
    ($trait_name:ident, $method:ident, $t:ty) => {
        impl $trait_name for $t {
            #[inline]
            fn $method(&self, v: &$t) -> Option<$t> {
                <$t>::$method(*self, *v)
            }
        }
    };
}

checked_impl!(CheckedAdd, checked_add, u8);
checked_impl!(CheckedAdd, checked_add, u16);
checked_impl!(CheckedAdd, checked_add, u32);
checked_impl!(CheckedAdd, checked_add, u64);

checked_impl!(CheckedAdd, checked_add, i8);
checked_impl!(CheckedAdd, checked_add, i16);
checked_impl!(CheckedAdd, checked_add, i32);
checked_impl!(CheckedAdd, checked_add, i64);

impl CheckedAdd for f32 {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        let sum = self + v;
        if sum.is_infinite() {
            None
        } else {
            Some(sum)
        }
    }
}

impl CheckedAdd for f64 {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        let sum = self + v;
        if sum.is_infinite() {
            None
        } else {
            Some(sum)
        }
    }
}

/// Performs multiplication that returns `None` instead of wrapping around on underflow or
/// overflow.
pub trait CheckedMul: Sized + Mul<Self, Output = Self> {
    /// Multiplies two numbers, checking for underflow or overflow. If underflow
    /// or overflow happens, `None` is returned.
    fn checked_mul(&self, v: &Self) -> Option<Self>;
}

checked_impl!(CheckedMul, checked_mul, u8);
checked_impl!(CheckedMul, checked_mul, u16);
checked_impl!(CheckedMul, checked_mul, u32);
checked_impl!(CheckedMul, checked_mul, u64);

checked_impl!(CheckedMul, checked_mul, i8);
checked_impl!(CheckedMul, checked_mul, i16);
checked_impl!(CheckedMul, checked_mul, i32);
checked_impl!(CheckedMul, checked_mul, i64);

impl CheckedMul for f32 {
    fn checked_mul(&self, v: &Self) -> Option<Self> {
        let mul = self * v;
        if mul.is_infinite() {
            None
        } else {
            Some(mul)
        }
    }
}

impl CheckedMul for f64 {
    fn checked_mul(&self, v: &Self) -> Option<Self> {
        let mul = self * v;
        if mul.is_infinite() {
            None
        } else {
            Some(mul)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    use super::{from_str, Deserializer};
    use crate::error::Error;

    #[test]
    fn test_boolean() {
        assert_eq!(from_str("true"), Ok(true));
        assert_eq!(from_str("false"), Ok(false));

        // error cases
        assert_eq!(from_str::<bool>("tru"), Err(Error::Eof));
        assert_eq!(from_str::<bool>("f"), Err(Error::Eof));
        assert_eq!(from_str::<bool>("treu"), Err(Error::ExpectedBoolean));
    }

    #[test]
    fn test_null() {
        assert_eq!(from_str("null"), Ok(()));
    }

    #[test]
    fn test_option() {
        assert_eq!(from_str::<Option<u32>>("null"), Ok(None));
        assert_eq!(from_str::<Option<u32>>("42"), Ok(Some(42)));

        // Errors
        assert_eq!(from_str::<Option<()>>("nul"), Err(Error::Eof));
        assert_eq!(from_str::<Option<()>>("nu1l"), Err(Error::ExpectedNull));
    }

    #[test]
    fn test_number_signed() {
        assert_eq!(from_str::<u64>("0"), Ok(0));
        assert_eq!(from_str::<u8>("123"), Ok(123));

        // TODO: Errors
        assert_eq!(from_str::<u8>("99999"), Err(Error::NumberOverflow));
    }

    #[test]
    fn test_number_unsigned() {
        assert_eq!(from_str::<i32>("0"), Ok(0));
        assert_eq!(from_str::<i16>("+17"), Ok(17));
        assert_eq!(from_str::<i32>("-98"), Ok(-98));
    }

    #[test]
    fn test_number_float() {
        assert_eq!(from_str::<f32>("0"), Ok(0.0));
        assert_eq!(from_str::<f32>("34.5"), Ok(34.5));
        assert_eq!(from_str::<f64>("-3.62"), Ok(-3.62));
        assert_eq!(from_str::<f64>("+123.6"), Ok(123.6));
        assert_eq!(from_str::<f32>("4."), Ok(4.0));
        assert_eq!(from_str::<f64>("-.002"), Ok(-0.002));
        assert_eq!(from_str::<f64>(".002"), Ok(0.002));
        assert_eq!(from_str::<f64>("+.002"), Ok(0.002));
        assert_eq!(from_str::<f64>("0.0"), Ok(0.0));

        // Errors
        assert_eq!(from_str::<u32>("-1"), Err(Error::ExpectedUnsignedInteger));
        assert_eq!(from_str::<u32>("1.3"), Err(Error::ExpectedFloat));
        assert_eq!(
            from_str::<f32>(&std::f64::MAX.to_string()),
            Err(Error::NumberOverflow)
        );
    }

    #[test]
    fn test_string() {
        assert_eq!(
            from_str(r#"(0ab\(\\fo\)?!\200)"#),
            Ok(r#"0ab(\fo)?!€"#.to_string())
        );
        assert_eq!(
            from_str(
                r#"(no new\
line)"#
            ),
            Ok("no newline".to_string())
        );
        assert_eq!(
            from_str(
                r#"(new
line)"#
            ),
            Ok("new\nline".to_string())
        );
    }

    #[test]
    fn test_seq() {
        assert_eq!(from_str("[1 2 3 4]"), Ok(vec![1, 2, 3, 4]));
        assert_eq!(
            from_str("[(a) (b)]"),
            Ok(vec![String::from("a"), String::from("b")])
        );
    }

    #[test]
    fn test_char() {
        assert_eq!(from_str("(a)"), Ok('a'));
    }

    #[test]
    fn test_name() {
        pub fn from_str(s: &str) -> String {
            let mut input = s.as_bytes();
            let mut deserializer = Deserializer::new(input);
            deserializer.parse_name().unwrap()
        }

        assert_eq!(&from_str("/Adobe#20Green"), "Adobe Green");
        assert_eq!(&from_str("/PANTONE#205757#20CV"), "PANTONE 5757 CV");
        assert_eq!(&from_str("/paired#28#29parentheses"), "paired()parentheses");
        assert_eq!(&from_str("/The_Key_of_F#23_Minor"), "The_Key_of_F#_Minor");
        assert_eq!(&from_str("/A#42"), "AB");
    }

    #[test]
    fn test_struct() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Test {
            int: u32,
            seq: Vec<String>,
        }

        let j = r#"<< /int 1 /seq [(a) (b)] >>"#;
        let expected = Test {
            int: 1,
            seq: vec!["a".to_owned(), "b".to_owned()],
        };
        assert_eq!(expected, from_str(j).unwrap());
    }

    #[test]
    fn test_map() {
        use std::collections::HashMap;

        let mut expected = HashMap::new();
        expected.insert("foo".to_string(), "bar".to_string());

        let j = r#"<< /foo (bar) >>"#;
        assert_eq!(expected, from_str(j).unwrap());
    }

    #[test]
    fn test_enum() {
        #[derive(Deserialize, PartialEq, Debug)]
        enum E {
            Unit,
            Newtype(u32),
            Tuple(u32, u32),
            Struct { a: u32 },
        }

        let j = "/Unit";
        let expected = E::Unit;
        assert_eq!(expected, from_str(j).unwrap());

        let j = "<< /Newtype 1 >>";
        let expected = E::Newtype(1);
        assert_eq!(expected, from_str(j).unwrap());

        let j = "<< /Tuple [1 2] >>";
        let expected = E::Tuple(1, 2);
        assert_eq!(expected, from_str(j).unwrap());

        let j = "<< /Struct << /a 1 >> >>";
        let expected = E::Struct { a: 1 };
        assert_eq!(expected, from_str(j).unwrap());
    }

    #[test]
    fn test_bytes() {
        let data = "foobar";
        let mut expected = data.as_bytes().to_vec();

        let j = r#"[102 111 111 98 97 114]"#;
        assert_eq!(expected, from_str::<'_, Vec<u8>>(j).unwrap());
    }
}
