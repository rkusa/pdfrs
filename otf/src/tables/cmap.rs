mod format12;
mod format4;

use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{self, Cursor};
use std::mem;
use std::sync::Arc;

use super::{FontData, FontTable, Glyph};
use crate::utils::limit_read::LimitRead;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use format12::Format12;
use format4::Format4;

// TODO: support subtable 14?

/// A font's CMAP table, which ddefines the mapping of character codes to the glyph index values
/// used in the font. Supported character encodings are (in the order of how they are used if they
/// are defined):
/// | platform ID | encoding ID |                                          |
/// |-------------|-------------|------------------------------------------|
/// | 0           | 4           | Unicode >= 2.0, non-BMP allowed          |
/// | 3           | 10          | Windows, full Unicode                    |
/// | 0           | 3           | Unicode >= 2.0, BMP only                 |
/// | 3           | 1           | Windows, compatbility with older devices |
///
/// Supported subtable formats are: 4 and 12
///
/// Not supported character encodings and subtable formats are ignored. An error is returned, if
/// there is not a single supported character encoding and subtable combination.
///
/// See OpenType sepc: https://docs.microsoft.com/en-us/typography/opentype/spec/cmap
#[derive(Debug, PartialEq, Clone)]
pub struct CmapTable {
    pub(crate) version: u16,
    pub(crate) encoding_records: Vec<EncodingRecord>,
}

impl<'a> FontTable<'a, (), (), ()> for CmapTable {
    fn name() -> &'static str {
        "cmap"
    }
}

impl<'a> FontData<'a> for CmapTable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        mut rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let offset = rd.position();

        let version = rd.read_u16::<BigEndian>()?;
        let num_tables = rd.read_u16::<BigEndian>()?;

        let mut raw_records = Vec::with_capacity(num_tables.min(4) as usize);
        for _ in 0..num_tables {
            let record = RawEncodingRecord::unpack(&mut rd, ())?;
            // skip unsupported formats
            if !matches!(
                (record.platform_id, record.encoding_id),
                (0, 4) | (0, 3) | (3, 10) | (3, 1)
            ) {
                continue;
            }
            raw_records.push(record);
        }

        if raw_records.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Font does not contain any supported CMAP",
            ));
        }

        let mut records: Vec<(u32, EncodingRecord)> = Vec::with_capacity(raw_records.len());
        for raw_record in &raw_records {
            let existing_subtable = records
                .iter()
                .find(|(offset, _)| raw_record.offset == *offset)
                .map(|(_, subtable)| Arc::clone(&subtable.subtable));
            if let Some(subtable) = existing_subtable {
                records.push((
                    raw_record.offset,
                    EncodingRecord {
                        platform_id: raw_record.platform_id,
                        encoding_id: raw_record.encoding_id,
                        subtable,
                    },
                ));
                continue;
            }

            rd.set_position(offset + (raw_record.offset) as u64);
            let subtable = Subtable::unpack(&mut rd, ())?;
            records.push((
                raw_record.offset,
                EncodingRecord {
                    platform_id: raw_record.platform_id,
                    encoding_id: raw_record.encoding_id,
                    subtable: Arc::new(subtable),
                },
            ));
        }
        let encoding_records = records.into_iter().map(|(_, st)| st).collect();
        Ok(CmapTable {
            version,
            encoding_records,
        })
    }

    fn pack<W: io::Write>(&self, mut wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        // cmap subtables
        let mut encoding_records_data = Vec::new();
        let mut raw_recods = Vec::with_capacity(self.encoding_records.len());

        // reserve cmap table data
        let mut subtable_offset = 4 + self.encoding_records.len() * 8;
        let mut written_subtables = Vec::new();
        for subtable in &self.encoding_records {
            let prev_offset = written_subtables
                .iter()
                .find(|(_, other)| Arc::ptr_eq(other, &subtable.subtable))
                .map(|(offset, _)| *offset);
            if let Some(prev_offset) = prev_offset {
                raw_recods.push(RawEncodingRecord {
                    platform_id: subtable.platform_id,
                    encoding_id: subtable.encoding_id,
                    offset: prev_offset,
                });
                continue;
            }

            let len_before = encoding_records_data.len();
            subtable.subtable.pack(&mut encoding_records_data, ())?; // align to 4 bytes
            let record_offset = u32::try_from(subtable_offset).ok().unwrap_or(u32::MAX);
            raw_recods.push(RawEncodingRecord {
                platform_id: subtable.platform_id,
                encoding_id: subtable.encoding_id,
                offset: record_offset,
            });
            written_subtables.push((record_offset, subtable.subtable.clone()));
            subtable_offset += encoding_records_data.len() - len_before;
        }

        wr.write_u16::<BigEndian>(self.version)?;
        wr.write_u16::<BigEndian>(self.encoding_records.len() as u16)?;
        for record in raw_recods {
            record.pack(&mut wr, ())?;
        }
        wr.write_all(&encoding_records_data)?;

        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _dep: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        let mut subsetted_subtables: Vec<(Arc<Subtable>, Arc<Subtable>)> = Vec::new();
        let encoding_records = self
            .encoding_records
            .iter()
            .map(|entry| {
                let new_subtable = subsetted_subtables
                    .iter()
                    .find(|(prev, _)| Arc::ptr_eq(prev, &entry.subtable))
                    .map(|(_, new_subtable)| new_subtable.clone())
                    .unwrap_or_else(|| {
                        let new_subtable =
                            Arc::new(entry.subtable.subset(&glyphs, ()).into_owned());
                        subsetted_subtables.push((entry.subtable.clone(), new_subtable.clone()));
                        new_subtable
                    });

                EncodingRecord {
                    platform_id: entry.platform_id,
                    encoding_id: entry.encoding_id,
                    subtable: new_subtable,
                }
            })
            .collect();

        Cow::Owned(CmapTable {
            version: self.version,
            encoding_records,
        })
    }
}

pub struct RawEncodingRecord {
    platform_id: u16,
    encoding_id: u16,
    /// Byte offset from beginning of table to the subtable for this encoding.
    offset: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct EncodingRecord {
    pub(crate) platform_id: u16,
    pub(crate) encoding_id: u16,
    pub(crate) subtable: Arc<Subtable>,
}

impl<'a> FontData<'a> for RawEncodingRecord {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        Ok(RawEncodingRecord {
            platform_id: rd.read_u16::<BigEndian>()?,
            encoding_id: rd.read_u16::<BigEndian>()?,
            offset: rd.read_u32::<BigEndian>()?,
        })
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        wr.write_u16::<BigEndian>(self.platform_id)?;
        wr.write_u16::<BigEndian>(self.encoding_id)?;
        wr.write_u32::<BigEndian>(self.offset)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Subtable {
    Format4(Format4),
    Format12(Format12),
}

impl Subtable {
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        match self {
            Subtable::Format4(subtable) => subtable.glyph_id(codepoint),
            Subtable::Format12(subtable) => subtable.glyph_id(codepoint),
        }
    }
}

impl<'a> FontData<'a> for Subtable {
    type UnpackDep = ();
    type PackDep = ();
    type SubsetDep = ();

    fn unpack<R: io::Read + AsRef<[u8]>>(
        rd: &mut Cursor<R>,
        _: Self::UnpackDep,
    ) -> Result<Self, io::Error> {
        let format = rd.read_u16::<BigEndian>()?;

        match format {
            4 => {
                let mut length = rd.read_u16::<BigEndian>()?;
                // length excluding format and length
                length -= (mem::size_of::<u16>() * 2) as u16;
                let mut rd = Cursor::new(LimitRead::from_cursor(rd, length as usize));
                Ok(Subtable::Format4(Format4::unpack(&mut rd, ())?))
            }
            12 => {
                rd.read_u16::<BigEndian>()?; // reserved
                let mut length = rd.read_u32::<BigEndian>()?;
                // length excluding format, reserved and length
                length -= (mem::size_of::<u16>() * 2 + mem::size_of::<u32>()) as u32;
                let mut rd = Cursor::new(LimitRead::from_cursor(rd, length as usize));
                Ok(Subtable::Format12(Format12::unpack(&mut rd, ())?))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("CMAP subtable format {} is not supported", format),
            )),
        }
    }

    fn pack<W: io::Write>(&self, wr: &mut W, _: Self::PackDep) -> Result<(), io::Error> {
        let mut buf = Vec::new();
        match self {
            Subtable::Format4(subtable) => subtable.pack(&mut buf, ())?,
            Subtable::Format12(subtable) => subtable.pack(&mut buf, ())?,
        }

        if buf.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("CMAP subtable cannot be bigger than {} bytes", u16::MAX),
            ));
        }

        match self {
            Subtable::Format4(_) => {
                wr.write_u16::<BigEndian>(4)?;
                // buf len + format and length
                wr.write_u16::<BigEndian>((buf.len() + mem::size_of::<u16>() * 2) as u16)?;
            }
            Subtable::Format12(_) => {
                wr.write_u16::<BigEndian>(12)?;
                // reserved
                wr.write_u16::<BigEndian>(0)?;
                // buf len + format, reserved and length
                wr.write_u32::<BigEndian>(
                    (buf.len() + mem::size_of::<u16>() * 2 + mem::size_of::<u32>()) as u32,
                )?;
            }
        }

        wr.write_all(&buf)?;
        Ok(())
    }

    fn subset(&'a self, glyphs: &[Glyph], _dep: Self::SubsetDep) -> Cow<'a, Self>
    where
        Self: Clone,
    {
        Cow::Owned(match self {
            Subtable::Format4(subtable) => {
                // Note: it could be checked here if the subset contains a code-point > u16:MAX and
                // if so to create a format 12 subset instead. However, if the font was initially
                // parsed as a format 4 font, it does not contain such code-codepoints. The
                // fallback to format 12 would only be necessary if the library is updated to
                // actively update/extend an existing font instead of just reading and subsetting.
                Subtable::Format4(subtable.subset(glyphs, ()).into_owned())
            }
            Subtable::Format12(subtable) => {
                Subtable::Format12(subtable.subset(glyphs, ()).into_owned())
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::OffsetTable;

    #[test]
    fn test_cmap_table_encode_decode() {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let cmap_table: CmapTable = table.unpack_required_table((), &mut cursor).unwrap();

        assert_eq!(cmap_table.version, 0);
        assert_eq!(cmap_table.encoding_records.len(), 4);

        assert_eq!(cmap_table.encoding_records[0].platform_id, 0);
        assert_eq!(cmap_table.encoding_records[0].encoding_id, 3);

        assert_eq!(cmap_table.encoding_records[1].platform_id, 0);
        assert_eq!(cmap_table.encoding_records[1].encoding_id, 4);

        assert_eq!(cmap_table.encoding_records[2].platform_id, 3);
        assert_eq!(cmap_table.encoding_records[2].encoding_id, 1);

        assert_eq!(cmap_table.encoding_records[3].platform_id, 3);
        assert_eq!(cmap_table.encoding_records[3].encoding_id, 10);

        // re-pack and compare
        let mut buffer = Vec::new();
        cmap_table.pack(&mut buffer, ()).unwrap();
        assert_eq!(
            CmapTable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
            cmap_table
        );
    }

    #[test]
    fn test_cmap_subtable_encode_decode() {
        let data = include_bytes!("../../../fonts/Iosevka/iosevka-regular.ttf").to_vec();
        let mut cursor = Cursor::new(&data[..]);
        let table = OffsetTable::unpack(&mut cursor, ()).unwrap();
        let cmap_table: CmapTable = table.unpack_required_table((), &mut cursor).unwrap();

        for record in &cmap_table.encoding_records {
            // re-pack and compare
            let mut buffer = Vec::new();
            record.subtable.pack(&mut buffer, ()).unwrap();
            assert_eq!(
                &Subtable::unpack(&mut Cursor::new(&buffer[..]), ()).unwrap(),
                record.subtable.as_ref()
            );
        }
    }
}
