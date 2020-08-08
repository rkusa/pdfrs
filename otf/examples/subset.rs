use std::fs::File;
use std::io;

use otf::OpenTypeFont;

fn main() -> Result<(), io::Error> {
    let data = include_bytes!("../tests/fonts/Iosevka/iosevka-regular.ttf");
    let font = OpenTypeFont::from_slice(&data[..])?;
    // let subset = font.subset("Grumpy wizards make toxic brew for the evil Queen and Jack.");
    let subset = font.subset("a");

    let file = File::create("iosevka-regular-subset.ttf")?;
    subset.to_writer(file)?;

    Ok(())
}
