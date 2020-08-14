use std::fs::File;
use std::ops::Deref;

use pdfrs::fonts::{FontCollection, HELVETICA};
use pdfrs::Document;
use pdfrs_macros::test as pdf_test;

fn afm_helvetica() -> impl FontCollection {
    HELVETICA.deref()
}

#[pdf_test("./fixtures/empty.pdf", afm_helvetica)]
async fn empty(doc: &mut Document<File>) {
    // just testing an empty document here
}

#[pdf_test("./fixtures/basic_text.pdf", afm_helvetica)]
async fn basic_text(doc: &mut Document<_, File>) {
    doc.text("Hello World", None).await.unwrap();
}
