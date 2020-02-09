mod afm;

use afm::AfmFont;

#[cfg(feature = "courier_bold")]
include!(concat!(env!("OUT_DIR"), "/courier_bold.rs"));
#[cfg(feature = "courier_bold_oblique")]
include!(concat!(env!("OUT_DIR"), "/courier_bold_oblique.rs"));
#[cfg(feature = "courier_oblique")]
include!(concat!(env!("OUT_DIR"), "/courier_oblique.rs"));
#[cfg(feature = "courier")]
include!(concat!(env!("OUT_DIR"), "/courier.rs"));
#[cfg(feature = "helvetica_bold")]
include!(concat!(env!("OUT_DIR"), "/helvetica_bold.rs"));
#[cfg(feature = "helvetica_bold_oblique")]
include!(concat!(env!("OUT_DIR"), "/helvetica_bold_oblique.rs"));
#[cfg(feature = "helvetica_oblique")]
include!(concat!(env!("OUT_DIR"), "/helvetica_oblique.rs"));
#[cfg(feature = "helvetica")]
include!(concat!(env!("OUT_DIR"), "/helvetica.rs"));
#[cfg(feature = "symbol")]
include!(concat!(env!("OUT_DIR"), "/symbol.rs"));
#[cfg(feature = "times_bold")]
include!(concat!(env!("OUT_DIR"), "/times_bold.rs"));
#[cfg(feature = "times_bold_italic")]
include!(concat!(env!("OUT_DIR"), "/times_bold_italic.rs"));
#[cfg(feature = "times_italic")]
include!(concat!(env!("OUT_DIR"), "/times_italic.rs"));
#[cfg(feature = "times_roman")]
include!(concat!(env!("OUT_DIR"), "/times_roman.rs"));
#[cfg(feature = "zapf_dingbats")]
include!(concat!(env!("OUT_DIR"), "/zapf_dingbats.rs"));
