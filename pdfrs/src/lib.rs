mod document;
mod page;
mod stream;

pub use document::Document;

// #[cfg(test)]
// mod tests {
//     use std::io;
//     use std::fs::File;
//     use crate::document::Document;

//     #[test]
//     fn it_works() {
//         let mut doc = Document::new();
//         let mut f = File::create("test.pdf").unwrap();

//         io::copy(&mut doc, &mut f).unwrap();
//     }
// }
