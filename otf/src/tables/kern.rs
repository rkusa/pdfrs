use std::convert::TryFrom;
use std::io::{self, Cursor};

use super::{FontData, FontTable};
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// The kerning table contains the values that control the inter-character spacing for the glyphs in
/// a font.
/// See spec:
/// - https://docs.microsoft.com/en-us/typography/opentype/spec/kern
/// - https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6kern.html
#[derive(Debug, PartialEq, Clone)]
pub struct KernTable {
    pub version: Version,
    pub subtables: Vec<Subtable>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Version {
    Windows,
    Mac,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Subtable {
    /// The type of information is contained in this table.
    pub coverage: Coverage,
    pub data: SubtableData,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Coverage {
    /// `true` if table is horizontal data, `false` if vertical.
    is_horizontal: bool,
    /// Whether the kerning is perpendicular to the flow of the text.
    has_cross_stream: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SubtableData {
    Format0(Format0),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Format0 {
    pairs: Vec<(u32, i16)>,
}

impl<'a> FontTable<'a, (), (), ()> for KernTable {
    fn name() -> &'static str {
        "kern"
    }
}

impl<'a> FontData<'a> for KernTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let version = match rd.read_u16::<BigEndian>()? {
            0 => Version::Windows,
            1 => Version::Mac,
            v => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Unsupported kern table version: {}", v),
                ))
            }
        };

        // skipping the second part of macOS' fixed32 version
        if version == Version::Mac {
            rd.read_u16::<BigEndian>()?;
        }

        let n_tables = match version {
            Version::Windows => rd.read_u16::<BigEndian>()? as usize,
            Version::Mac => rd.read_u32::<BigEndian>()? as usize,
        };

        let mut subtables = Vec::with_capacity(n_tables);
        for _ in 0..n_tables {
            subtables.push(Subtable::unpack(&mut rd, version)?);
        }

        Ok(KernTable { version, subtables })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        match self.version {
            Version::Windows => {
                wr.write_u16::<BigEndian>(0)?; // version
                wr.write_u16::<BigEndian>(u16::try_from(self.subtables.len()).map_err(|_| {
                    super::error(format!(
                        "Cannot write more than {} kern subtables",
                        u16::MAX
                    ))
                })?)?;
                // n_tables
            }
            Version::Mac => {
                wr.write_u16::<BigEndian>(1)?; // version
                wr.write_u16::<BigEndian>(0)?; // version
                wr.write_u32::<BigEndian>(u32::try_from(self.subtables.len()).map_err(|_| {
                    super::error(format!(
                        "Cannot write more than {} kern subtables",
                        u32::MAX
                    ))
                })?)?; // n_tables
            }
        }

        for subtable in &self.subtables {
            subtable.pack(&mut wr, self.version)?;
        }

        Ok(())
    }
}

impl<'a> FontData<'a> for Subtable {
    type UnpackDep = Version;
    type PackDep = Version;
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        version: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let (length, format, coverage) = match version {
            Version::Windows => {
                let version = rd.read_u16::<BigEndian>()?;
                if version != 0 {
                    return Err(super::error(format!(
                        "Kern subtable version {} is not supported",
                        version
                    )));
                }
                let length = rd.read_u16::<BigEndian>()?;
                let format = rd.read_u8()?;
                let coverage = rd.read_u8()?;
                let coverage = Coverage {
                    is_horizontal: coverage & 1 != 0,
                    has_cross_stream: coverage & 4 != 0,
                };
                ((length.saturating_sub(6)) as usize, format, coverage)
            }
            Version::Mac => {
                let length = rd.read_u32::<BigEndian>()?;
                let coverage = rd.read_u8()?;
                let format = rd.read_u8()?;
                let coverage = Coverage {
                    is_horizontal: coverage & 0x80 == 0,
                    has_cross_stream: coverage & 0x40 != 0,
                };
                let _tuple_index = rd.read_u16::<BigEndian>()?;
                (length.saturating_sub(8) as usize, format, coverage)
            }
        };

        let mut rd = Cursor::new(LimitRead::from_cursor(rd, length));
        let data = match format {
            0 => SubtableData::Format0(Format0::unpack(&mut rd, ())?),
            format => {
                return Err(super::error(format!(
                    "Kern subtable format {} is not yet supported",
                    format
                )))
            }
        };

        Ok(Subtable { coverage, data })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, version: Self::PackDep) -> Result<(), io::Error> {
        let format: u8 = match &self.data {
            SubtableData::Format0(_) => 0,
        };
        match version {
            Version::Windows => {
                wr.write_u16::<BigEndian>(0)?; // version
                                               // TODO
                wr.write_u16::<BigEndian>(0)?; // length
                wr.write_u8(format)?;
                let mut coverage = 0;
                if self.coverage.is_horizontal {
                    coverage |= 1;
                }
                if self.coverage.has_cross_stream {
                    coverage |= 4;
                }
                wr.write_u8(coverage)?;
            }
            Version::Mac => {
                // TODO
                wr.write_u32::<BigEndian>(0)?; // length
                wr.write_u8(format)?;
                let mut coverage = 0;
                if !self.coverage.is_horizontal {
                    coverage |= 0x80;
                }
                if self.coverage.has_cross_stream {
                    coverage |= 0x40;
                }
                wr.write_u8(coverage)?;
            }
        }

        match &self.data {
            SubtableData::Format0(format0) => format0.pack(wr, ())?,
        }

        Ok(())
    }
}

impl<'a> FontData<'a> for Format0 {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let n_pairs = rd.read_u16::<BigEndian>()? as usize;
        rd.read_u16::<BigEndian>()?; // search_range
        rd.read_u16::<BigEndian>()?; // entry_selector
        rd.read_u16::<BigEndian>()?; // range_shift

        let mut pairs = Vec::with_capacity(n_pairs);
        for _ in 0..n_pairs {
            pairs.push((rd.read_u32::<BigEndian>()?, rd.read_i16::<BigEndian>()?));
        }

        Ok(Format0 { pairs })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        let n_pairs = u16::try_from(self.pairs.len()).map_err(|_| {
            super::error(format!(
                "Cannot write more than {} kern subtable format0 entries",
                u16::MAX
            ))
        })?;
        let x = 2u16.pow((n_pairs as f32).log2() as u32);
        let search_range = x * 16;
        let entry_selector = (x as f32).log2() as u16;
        let range_shift = n_pairs * 16 - search_range;

        wr.write_u16::<BigEndian>(n_pairs)?;
        wr.write_u16::<BigEndian>(search_range)?;
        wr.write_u16::<BigEndian>(entry_selector)?;
        wr.write_u16::<BigEndian>(range_shift)?;

        for (pair, kerning) in &self.pairs {
            wr.write_u32::<BigEndian>(*pair)?;
            wr.write_i16::<BigEndian>(*kerning)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    // TODO: find font with kern table for testing
}
