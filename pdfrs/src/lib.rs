#[macro_use]
extern crate lazy_static;

mod document;
pub mod fonts;
mod idseq;
mod layout;
mod page;
mod stream;
mod text;
mod writer;

pub use document::Document;
pub use pdfrs_macros::test;
