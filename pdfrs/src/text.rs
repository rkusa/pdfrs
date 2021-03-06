use std::collections::HashSet;
use std::io;

use crate::fonts::{Font, SubsetRef};
use crate::stream::Stream;
use async_std::io::prelude::Write;

// TODO: non-Arc font
pub async fn write_text<W: Write + Unpin>(
    text: &str,
    font: &dyn Font,
    wr: &mut Stream<W>,
) -> Result<HashSet<SubsetRef>, io::Error> {
    wr.begin_text().await?;
    wr.set_text_matrix(1.0, 0.0, 0.0, 1.0, 10.0, 821.721)
        .await?;
    wr.set_text_leading(10.175).await?;
    wr.set_fill_color(0.0, 0.0, 0.0).await?;
    let subset_refs = wr.show_text_string(text, font, 11.0).await?;
    wr.end_text().await?;

    Ok(subset_refs)
}
