use std::io::{self};

use crate::stream::Stream;
use async_std::io::prelude::{Write, WriteExt};

// TODO: non-Arc font
pub async fn write_text<W: Write + Unpin>(
    wr: &mut Stream<W>,
    _text: &str,
    font_id: usize,
) -> Result<(), io::Error> {
    wr.begin_text().await?;
    wr.set_text_matrix(1.0, 0.0, 0.0, 1.0, 10.0, 821.721)
        .await?;
    wr.set_text_leading(10.175).await?;
    wr.set_text_font(font_id, 11.0).await?;
    wr.set_fill_color(0.0, 0.0, 0.0).await?;
    writeln!(wr, "[(Hello) -278.000 (W) 30.000 (or) -15.000 (ld)] TJ").await?;
    wr.end_text().await?;

    Ok(())
}
