mod document;
pub mod fonts;
mod idseq;
mod layout;
mod page;
mod stream;
mod text;
mod writer;

use std::ops::Deref;

pub use document::{Document, DocumentBuilder};
use fonts::FontCollection;
use js_sys::Uint8Array;
use pdfrs_afm::HELVETICA;
use wasm_bindgen::prelude::*;

// #[wasm_bindgen]
// extern "C" {
//     async fn basic_afm_document() -> Array;
// }

#[wasm_bindgen]
pub async fn basic_afm_document() -> Uint8Array {
    let mut result = Vec::new();
    let mut doc = Document::builder(afm_helvetica())
        .start(&mut result)
        .await
        .unwrap();

    doc.text("Hello World", None).await.unwrap();
    doc.end().await.unwrap();

    // ByteStream::new(&result)
    unsafe { Uint8Array::view(&result) }
}

fn afm_helvetica() -> impl FontCollection {
    HELVETICA.deref()
}

// https://github.com/rustwasm/wasm-bindgen/issues/111#issuecomment-455268735
// const texture = render();
// const textureRaw = new Uint8ClampedArray(memory.buffer, texture.offset(), texture.size());
// const image = new ImageData(textureRaw, width, height);

#[wasm_bindgen]
pub struct ByteStream {
    offset: *const u8,
    size: usize,
}

#[wasm_bindgen]
impl ByteStream {
    pub fn new(bytes: &[u8]) -> ByteStream {
        ByteStream {
            offset: bytes.as_ptr(),
            size: bytes.len(),
        }
    }

    pub fn offset(&self) -> *const u8 {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
