mod paragraph;
mod style;

// TODO: remove allow(unused)
#[allow(unused)]
pub enum Render<'a> {
    Line { words: Vec<&'a str> },
    PageBreak,
    BlockEnd { y: f32 },
}
