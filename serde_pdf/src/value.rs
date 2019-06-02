use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::io;
use std::mem;
use std::str;

use serde::de::DeserializeOwned;
use serde::ser::{self, Serialize};

use self::ser::Serializer;

/// Represents any valid PDF value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Dictionary(HashMap<String, Value>),
}

impl Default for Value {
    fn default() -> Value {
        Value::Null
    }
}

impl ser::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(v) => serializer.serialize_bool(v),
            Value::Integer(v) => serializer.serialize_i64(v),
            Value::Float(v) => serializer.serialize_f64(v),
            Value::String(ref v) => serializer.serialize_str(v),
            Value::Array(ref v) => v.serialize(serializer),
            Value::Dictionary(ref v) => v.serialize(serializer),
        }
    }
}
