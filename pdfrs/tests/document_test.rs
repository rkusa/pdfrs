use pdfrs::fonts::HELVETICA;
use pdfrs::Document;
use std::fs::File;

#[pdfrs::test("./fixtures/empty.pdf")]
fn empty(doc: &mut Document<File>) {
    // just testing an empty document here
}

#[pdfrs::test("./fixtures/basic_text.pdf")]
fn basic_text(doc: &mut Document<File>) {
    // TODO: return result

    // just testing an empty document here
    doc.text("Works", &*HELVETICA).unwrap();
}
