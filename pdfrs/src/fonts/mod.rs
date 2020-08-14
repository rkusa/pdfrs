//! This module contains built-in PDF (AFM) fonts. When using those fonts, it is not necessary
//! to embed a font into the document, since each PDF reader provides them. However, compared to
//! embedding fonts, those AFM fonts only have a limited set of available characters.
//!
//! Each AFM font needs to be enabled via a Cargo feature. Only `HELVETICA` is enabled as part of
//! the default feature set. The following features for enabling AFM fonts are available:
//! `courier_bold`, `courier_bold_oblique`, `courier_oblique`, `courier`, `helvetica_bold`,
//! `helvetica_bold_oblique`, `helvetica_oblique`, `helvetica`, `symbol`, `times_bold`,
//! `times_bold_italic`, `times_italic`, `times_roman`, `zapf_dingbats`.

#[cfg(any(feature = "afm", test))]
pub mod afm;
mod font;
// mod otf;

// pub use self::otf::OpenTypeFont;
pub use font::{Font, FontCollection, SubsetRef};
#[cfg(any(feature = "afm", test))]
pub use pdfrs_afm::*;
