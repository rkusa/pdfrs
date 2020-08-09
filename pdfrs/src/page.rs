use std::collections::HashMap;

use crate::stream::StreamRef;
use serde::Serialize;
use serde_pdf::Reference;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Pages<'a> {
    pub media_box: (f64, f64, f64, f64),
    pub kids: Vec<Reference<Page<'a>>>,
    pub count: usize,
}

pub type FontRef = ();

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Resources<'a> {
    pub proc_set: Vec<&'a str>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub font: HashMap<String, Reference<FontRef>>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Page<'a> {
    pub parent: Reference<Pages<'a>>,
    pub resources: Resources<'a>,
    pub contents: Vec<Reference<StreamRef>>,
}
