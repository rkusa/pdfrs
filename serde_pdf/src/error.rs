use std::fmt::{self, Display};
use std::io;

use serde::{de, ser};

pub type Result<T> = std::result::Result<T, Error>;

// This is a bare-bones implementation. A real library would provide additional
// information in its error type, for example the line and column at which the
// error occurred, the byte offset into the input, or the current key being
// processed.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    // One or more variants that can be created by data structures through the
    // `ser::Error` and `de::Error` traits. For example the Serialize impl for
    // Mutex<T> might return an error because the mutex is poisoned, or the
    // Deserialize impl for a struct may return an error because a required
    // field is missing.
    Message(String),

    // TODO: keep error source around?
    Io(io::ErrorKind),

    // Zero or more variants that can be created directly by the Serializer and
    // Deserializer without going through `ser::Error` and `de::Error`. These
    // are specific to the format, in this case JSON.
    Eof,
    Syntax,
    ExpectedBoolean,
    ExpectedInteger,
    ExpectedUnsignedInteger,
    ExpectedFloat,
    ExpectedString,
    ExpectedName,
    ExpectedChar,
    ExpectedNull,
    ExpectedArray,
    ExpectedArrayComma,
    ExpectedArrayEnd,
    ExpectedMap,
    ExpectedMapColon,
    ExpectedMapComma,
    ExpectedMapEnd,
    ExpectedEnum,
    TrailingCharacters,
    NumberOverflow,
    InvalidEscapeSequence,
    MapKeyMustBeAString,
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Error::Message(ref msg) => msg,
            Error::Io(_) => "IO error",
            Error::Eof => "unexpected end of input",
            Error::Syntax => "Syntax (TODO)",
            Error::ExpectedBoolean => "ExpectedBoolean (TODO)",
            Error::ExpectedInteger => "ExpectedInteger (TODO)",
            Error::ExpectedUnsignedInteger => "ExpectedUnsignedInteger (TODO)",
            Error::ExpectedFloat => "ExpectedFloat (TODO)",
            Error::ExpectedString => "ExpectedString (TODO)",
            Error::ExpectedName => "ExpectedName (TODO)",
            Error::ExpectedChar => "ExpectedChar (TODO)",
            Error::ExpectedNull => "ExpectedNull (TODO)",
            Error::ExpectedArray => "ExpectedArray (TODO)",
            Error::ExpectedArrayComma => "ExpectedArrayComma (TODO)",
            Error::ExpectedArrayEnd => "ExpectedArrayEnd (TODO)",
            Error::ExpectedMap => "ExpectedMap (TODO)",
            Error::ExpectedMapColon => "ExpectedMapColon (TODO)",
            Error::ExpectedMapComma => "ExpectedMapComma (TODO)",
            Error::ExpectedMapEnd => "ExpectedMapEnd (TODO)",
            Error::ExpectedEnum => "ExpectedEnum (TODO)",
            Error::TrailingCharacters => "TrailingCharacters (TODO)",
            Error::NumberOverflow => "Number overflow",
            Error::InvalidEscapeSequence => "Invalid escape sequence in string literal",
            Error::MapKeyMustBeAString => "Map key must be a string",
        })
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err.kind())
    }
}

impl Into<io::Error> for Error {
    fn into(self) -> io::Error {
        match self {
            Error::Io(kind) => kind.into(),
            err => io::Error::new(io::ErrorKind::Other, err),
        }
    }
}
