use pdfrs::Document;
use std::fs::File;

#[pdfrs::test("./fixtures/empty.pdf")]
fn empty(doc: &mut Document<File>) {
    // just testing an empty document here
}
