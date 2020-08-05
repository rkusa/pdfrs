use std::io;

use super::{Serializer, NAME_RAW};
use crate::error::Error;
use chrono::prelude::*;
use serde::ser;

pub fn serialize<S, Tz>(date: &DateTime<Tz>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let s = date.format("%Y%m%d%H%M%S").to_string();
    let mut tz = date.format("%z").to_string();
    let min = tz.split_off(3);
    serializer.serialize_newtype_struct(NAME_RAW, &format!("(D:{}{}'{}')", s, tz, min))
}

#[test]
fn datetime_serialization() {
    use chrono::FixedOffset;

    #[derive(Serialize)]
    struct Test {
        #[serde(with = "crate::datetime")]
        datetime: DateTime<FixedOffset>,
    }

    let test = Test {
        datetime: FixedOffset::east(3600).ymd(2015, 2, 19).and_hms(22, 33, 26),
    };

    assert_eq!(
        crate::ser::to_string(&test).unwrap(),
        "<<\n\t/Type /Test\n\t/datetime (D:20150219223326+01'00')\n>>"
    );
}
