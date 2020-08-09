use std::io::{self};

use crate::fonts::Font;
use crate::stream::Stream;
use async_std::io::prelude::Write;

// TODO: non-Arc font
pub async fn write_text<W: Write + Unpin>(
    wr: &mut Stream<W>,
    text: &str,
    font_id: usize,
    font: &Font,
) -> Result<(), io::Error> {
    wr.begin_text().await?;
    wr.set_text_matrix(1.0, 0.0, 0.0, 1.0, 10.0, 821.721)
        .await?;
    wr.set_text_leading(10.175).await?;
    wr.set_text_font(font_id, 11.0).await?;
    wr.set_fill_color(0.0, 0.0, 0.0).await?;
    wr.show_text_string(text, font).await?;
    wr.end_text().await?;

    Ok(())
}
