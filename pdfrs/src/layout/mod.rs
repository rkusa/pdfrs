mod paragraph;
mod style;

pub enum Render<'a> {
    Line { words: Vec<&'a str> },
    PageBreak,
    BlockEnd { y: f32 },
}
