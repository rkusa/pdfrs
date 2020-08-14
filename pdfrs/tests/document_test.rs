use std::fs::File;
use std::ops::Deref;

use pdfrs::fonts::{FontCollection, OpenTypeFont, HELVETICA};
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

#[pdf_test(
    "./fixtures/basic_text_compressed.pdf",
    afm_helvetica,
    compressed = true
)]
async fn basic_text_compressed(doc: &mut Document<_, File>) {
    doc.text("Hello World", None).await.unwrap();
}

fn iosevka_regular() -> impl FontCollection {
    let data = include_bytes!("./fonts/Iosevka/iosevka-regular.ttf");
    OpenTypeFont::from_slice(&data[..]).unwrap()
}

#[pdf_test("./fixtures/basic_opentype_text.pdf", iosevka_regular)]
async fn basic_opentype_text(doc: &mut Document<_, File>) {
    doc.text("Hello World", None).await.unwrap();
    // doc.text("Hello World / Ⓗⓔⓛⓛⓞ Ⓦⓞⓡⓛⓓ", None).await.unwrap();
}
