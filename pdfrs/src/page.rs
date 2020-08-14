use std::collections::HashMap;

use crate::stream::StreamRef;
use serde::Serialize;
use serde_pdf::Reference;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Pages {
    pub media_box: (f64, f64, f64, f64),
    pub kids: Vec<Reference<Page>>,
    pub count: usize,
}

pub type FontRef = ();

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Resources {
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub font: HashMap<String, Reference<FontRef>>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Page {
    pub parent: Reference<Pages>,
    pub resources: Resources,
    pub contents: Vec<Reference<StreamRef>>,
}
