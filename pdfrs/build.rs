use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use inflector::Inflector;
use itertools::Itertools;
use regex::Regex;

fn main() -> io::Result<()> {
    // winansi_characters.txt source: https://github.com/prawnpdf/prawn
    let name_to_code = include_str!("./fonts/winansi_characters.txt")
        .split_whitespace()
        .enumerate()
        .map(|(i, c)| (c, i as u32))
        .collect::<HashMap<_, _>>();

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    #[cfg(feature = "courier_bold")]
    build_font(
        &name_to_code,
        "COURIER_BOLD",
        include_str!("./fonts/Courier-Bold.afm"),
        out_dir.join("courier_bold.rs"),
    )?;
    #[cfg(feature = "courier_bold_oblique")]
    build_font(
        &name_to_code,
        "COURIER_BOLD_OBLIQUE",
        include_str!("./fonts/Courier-BoldOblique.afm"),
        out_dir.join("courier_bold_oblique.rs"),
    )?;
    #[cfg(feature = "courier_oblique")]
    build_font(
        &name_to_code,
        "COURIER_OBLIQUE",
        include_str!("./fonts/Courier-Oblique.afm"),
        out_dir.join("courier_oblique.rs"),
    )?;
    #[cfg(feature = "courier")]
    build_font(
        &name_to_code,
        "COURIER",
        include_str!("./fonts/Courier.afm"),
        out_dir.join("courier.rs"),
    )?;

    #[cfg(feature = "helvetica_bold")]
    build_font(
        &name_to_code,
        "HELVETICA_BOLD",
        include_str!("./fonts/Helvetica-Bold.afm"),
        out_dir.join("helvetica_bold.rs"),
    )?;
    #[cfg(feature = "helvetica_bold_oblique")]
    build_font(
        &name_to_code,
        "HELVETICA_BOLD_OBLIQUE",
        include_str!("./fonts/Helvetica-BoldOblique.afm"),
        out_dir.join("helvetica_bold_oblique.rs"),
    )?;
    #[cfg(feature = "helvetica_oblique")]
    build_font(
        &name_to_code,
        "HELVETICA_OBLIQUE",
        include_str!("./fonts/Helvetica-Oblique.afm"),
        out_dir.join("helvetica_oblique.rs"),
    )?;
    #[cfg(feature = "helvetica")]
    build_font(
        &name_to_code,
        "HELVETICA",
        include_str!("./fonts/Helvetica.afm"),
        out_dir.join("helvetica.rs"),
    )?;

    #[cfg(feature = "symbol")]
    build_font(
        &name_to_code,
        "SYMBOL",
        include_str!("./fonts/Symbol.afm"),
        out_dir.join("symbol.rs"),
    )?;

    #[cfg(feature = "times_bold")]
    build_font(
        &name_to_code,
        "TIMES_BOLD",
        include_str!("./fonts/Times-Bold.afm"),
        out_dir.join("times_bold.rs"),
    )?;
    #[cfg(feature = "times_bold_italic")]
    build_font(
        &name_to_code,
        "TIMES_BOLD_ITALIC",
        include_str!("./fonts/Times-BoldItalic.afm"),
        out_dir.join("times_bold_italic.rs"),
    )?;
    #[cfg(feature = "times_italic")]
    build_font(
        &name_to_code,
        "TIMES_ITALIC",
        include_str!("./fonts/Times-Italic.afm"),
        out_dir.join("times_italic.rs"),
    )?;
    #[cfg(feature = "times_roman")]
    build_font(
        &name_to_code,
        "TIMES_ROMAN",
        include_str!("./fonts/Times-Roman.afm"),
        out_dir.join("times_roman.rs"),
    )?;

    #[cfg(feature = "zapf_dingbats")]
    build_font(
        &name_to_code,
        "ZAPF_DINGBATS",
        include_str!("./fonts/ZapfDingbats.afm"),
        out_dir.join("zapf_dingbats.rs"),
    )?;

    Ok(())
}

fn build_font(
    name_to_code: &HashMap<&str, u32>,
    name: &str,
    afm: &str,
    out_path: PathBuf,
) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(&out_path)?);

    writeln!(out, "lazy_static! {{")?;
    writeln!(out, "#[allow(clippy::needless_update)]")?;
    writeln!(out, "pub static ref {}: AfmFont = AfmFont {{", name)?;

    let mut parsing_char_metrics = 0;
    let mut parsing_kern_pairs = 0;
    let mut has_kern_paris = false;

    // e.g.: C 32 ; WX 278 ; N space ; B 0 0 0 0 ;
    let re_char_metrics =
        Regex::new(r"^C -?\d+ ; WX (?P<width>\d+) ; N (?P<name>\.?\w+) ;").unwrap();

    // e.g.: KPX o comma -40
    let re_kerning =
        Regex::new(r"^KPX (?P<left>\.?\w+) (?P<right>\.?\w+) (?P<width>-?\d+)$").unwrap();

    for line in afm.lines() {
        if parsing_char_metrics > 0 {
            parsing_char_metrics -= 1;

            let caps = re_char_metrics.captures(&line).unwrap();
            let name = caps.name("name").unwrap().as_str();
            let width = caps.name("width").unwrap().as_str().parse::<u32>().unwrap();
            if let Some(code) = name_to_code.get(name) {
                writeln!(out, "        ({}, {}),", code, width)?;
            }

            continue;
        }

        if parsing_kern_pairs > 0 {
            parsing_kern_pairs -= 1;

            let caps = re_kerning.captures(&line).unwrap();
            let left = caps.name("left").unwrap().as_str();
            let right = caps.name("right").unwrap().as_str();
            let width = caps.name("width").unwrap().as_str().parse::<i32>().unwrap();

            if let (Some(left), Some(right)) = (name_to_code.get(left), name_to_code.get(right)) {
                writeln!(out, "        (({}, {}), {}),", left, right, width)?;
            }

            continue;
        }

        if let Some((key, val)) = line.splitn(2, ' ').next_tuple() {
            match key {
                "StartCharMetrics" => {
                    parsing_char_metrics = val.parse().unwrap();
                    writeln!(out, "    glyph_widths: vec![")?;
                }
                "StartKernPairs" => {
                    parsing_kern_pairs = val.parse().unwrap();
                    has_kern_paris = true;
                    writeln!(out, "    kerning: vec![")?;
                }
                "ItalicAngle" => {
                    let key = key.to_snake_case();
                    writeln!(out, "    {}: {} as f32,", key, val)?;
                }
                "CapHeight" | "XHeight" | "Ascender" | "Descender" | "UnderlinePosition"
                | "UnderlineThickness" => {
                    let key = key.to_snake_case();
                    writeln!(out, "    {}: {},", key, val)?;
                }
                "FontName" | "FullName" | "FamilyName" | "CharacterSet" => {
                    let key = key.to_snake_case();
                    writeln!(out, "    {}: \"{}\",", key, val)?;
                }
                "FontBBox" => {
                    let val = val.replace(' ', ", ");
                    writeln!(out, "    font_bbox: ({}),", val)?;
                }
                _ => {}
            }
        } else {
            match line {
                "EndCharMetrics" => {
                    writeln!(out, "    ].into_iter().collect(),")?;
                }
                "EndKernPairs" => {
                    writeln!(out, "    ].into_iter().collect(),")?;
                }
                _ => {}
            }
        }
    }

    if !has_kern_paris {
        writeln!(out, "    kerning: std::collections::HashMap::new(),")?;
    }

    writeln!(out, "    ..Default::default()")?;
    writeln!(out, "}};")?;
    writeln!(out, "}}")?;

    Ok(())
}
