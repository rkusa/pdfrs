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

#[pdf_test("./fixtures/basic_afm_text.pdf", afm_helvetica)]
async fn basic_afm_text(doc: &mut Document<_, File>) {
    doc.text("Hello World", None).await.unwrap();
}

#[pdf_test("./fixtures/basic_compressed.pdf", afm_helvetica, compressed = true)]
async fn basic_compressed(doc: &mut Document<_, File>) {
    doc.text("Hello World", None).await.unwrap();
}

fn iosevka_regular() -> impl FontCollection {
    let data = include_bytes!("../../fonts/Iosevka/iosevka-regular.ttf");
    OpenTypeFont::from_slice(&data[..]).unwrap()
}

#[pdf_test("./fixtures/basic_monospaced_otf_text.pdf", iosevka_regular)]
async fn basic_monospaced_otf_text(doc: &mut Document<_, File>) {
    doc.text("Hello World — Ⓗⓔⓛⓛⓞ Ⓦⓞⓡⓛⓓ", None).await.unwrap();
}

fn source_sans_pro_regular() -> impl FontCollection {
    let data = include_bytes!("../../fonts/SourceSansPro/SourceSansPro-Regular.ttf");
    OpenTypeFont::from_slice(&data[..]).unwrap()
}

#[pdf_test("./fixtures/basic_proportional_otf_text.pdf", source_sans_pro_regular)]
async fn basic_proportional_otf_text(doc: &mut Document<_, File>) {
    doc.text("Hello World — Привет мир", None).await.unwrap();
}
