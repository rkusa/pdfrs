use std::fmt;

use crate::fonts::Font;

pub struct Style<'a> {
    pub font: &'a Font,
}

impl<'a> PartialEq for Style<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.font.base_name() == other.font.base_name()
    }
}

impl<'a> fmt::Debug for Style<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Style {{ font: {} }}", self.font.base_name())
    }
}
