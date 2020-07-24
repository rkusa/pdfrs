use pdfrs::fonts::HELVETICA;
use pdfrs::Document;
use pdfrs_macros::test as pdf_test;
use std::fs::File;

#[pdf_test("./fixtures/empty.pdf")]
fn empty(doc: &mut Document<File>) {
    // just testing an empty document here
}

#[pdf_test("./fixtures/basic_text.pdf")]
fn basic_text(doc: &mut Document<File>) {
    // TODO: return result

    // just testing an empty document here
    doc.text("Works", &*HELVETICA).unwrap();
}
