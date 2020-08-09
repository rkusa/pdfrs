use pdfrs::fonts::HELVETICA;
use pdfrs::Document;
use pdfrs_macros::test as pdf_test;
use std::fs::File;

#[pdf_test("./fixtures/empty.pdf", HELVETICA)]
async fn empty(doc: &mut Document<File>) {
    // just testing an empty document here
}

#[pdf_test("./fixtures/basic_text.pdf", HELVETICA)]
async fn basic_text(doc: &mut Document<_, File>) {
    doc.text("Hello World", None).await.unwrap();
}
