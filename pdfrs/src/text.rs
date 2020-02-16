use std::io::{self, Write};

use crate::stream::Stream;

// TODO: non-Arc font
pub fn write_text<W: io::Write>(
    wr: &mut Stream<W>,
    _text: &str,
    font_id: usize,
) -> Result<(), io::Error> {
    wr.begin_text()?;
    wr.set_text_matrix(1.0, 0.0, 0.0, 1.0, 10.0, 821.721)?;
    wr.set_text_leading(10.175)?;
    wr.set_text_font(font_id, 11.0)?;
    wr.set_fill_color(0.0, 0.0, 0.0)?;
    writeln!(wr, "[(Hello) -278.000 (W) 30.000 (or) -15.000 (ld)] TJ")?;
    wr.end_text()?;

    Ok(())
}
