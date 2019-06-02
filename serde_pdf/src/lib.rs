#![allow(unused)]

#[cfg(test)]
#[macro_use]
extern crate serde_derive;

mod de;
mod error;
mod object;
mod ser;
mod stream;
mod string;
mod value;

pub use crate::de::{from_str, Deserializer};
pub use crate::error::{Error, Result};
pub use crate::object::{Object, ObjectId, Reference};
pub use crate::ser::{datetime, to_string, to_writer};
pub use crate::stream::Stream;
pub use crate::string::{PdfStr, PdfString};
pub use crate::value::Value;
