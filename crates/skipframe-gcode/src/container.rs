//! Container layer: turns a path into a line reader over plain G-code.
//!
//! The parser never opens files itself. Adding a new container means adding one arm to
//! [`Container::of`] and one arm to [`with_reader`] -- notably `.bgcode` (PrusaSlicer's binary
//! format), which is out of scope for this release but already has its slot here.

use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::Path;

use crate::error::{Error, Result};

const READ_BUFFER: usize = 1 << 20; // 1 MiB; G-code lines are tiny and reads dominate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// `.gcode`, `.gco`, `.g` -- plain text, streamed straight off disk.
    Plain,
    /// `.gcode.3mf` -- zip archive with `Metadata/plate_N.gcode` inside.
    ThreeMf,
    /// `.bgcode` -- PrusaSlicer binary G-code. Recognised, not yet decoded.
    BinaryGcode,
}

impl Container {
    pub fn of(path: &Path) -> Result<Container> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::UnsupportedContainer("file has no readable name".into()))?
            .to_ascii_lowercase();

        if name.ends_with(".3mf") {
            Ok(Container::ThreeMf)
        } else if name.ends_with(".bgcode") {
            Ok(Container::BinaryGcode)
        } else if name.ends_with(".gcode") || name.ends_with(".gco") || name.ends_with(".g") {
            Ok(Container::Plain)
        } else {
            Err(Error::UnsupportedContainer(name))
        }
    }
}

/// Plate numbers available inside a `.gcode.3mf`. Plain files always report `[1]`.
pub fn list_plates(path: &Path) -> Result<Vec<u32>> {
    match Container::of(path)? {
        Container::Plain => Ok(vec![1]),
        Container::BinaryGcode => Err(Error::UnsupportedContainer(".bgcode".into())),
        Container::ThreeMf => {
            let mut zip = zip::ZipArchive::new(File::open(path)?)?;
            let mut plates: Vec<u32> = (0..zip.len())
                .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
                .filter_map(|n| plate_number(&n))
                .collect();
            plates.sort_unstable();
            plates.dedup();
            if plates.is_empty() {
                return Err(Error::NoGcodeInArchive);
            }
            Ok(plates)
        }
    }
}

/// Hand a buffered reader over the plain G-code text to `f`.
///
/// Plain files stream off disk with no intermediate copy. A `.gcode.3mf` entry is inflated into
/// memory first, because the zip crate's entry reader borrows the archive.
pub fn with_reader<T>(
    path: &Path,
    plate: Option<u32>,
    f: impl FnOnce(&mut dyn BufRead) -> Result<T>,
) -> Result<(T, Option<u32>)> {
    match Container::of(path)? {
        Container::Plain => {
            let mut reader = BufReader::with_capacity(READ_BUFFER, File::open(path)?);
            Ok((f(&mut reader)?, None))
        }
        Container::BinaryGcode => Err(Error::UnsupportedContainer(".bgcode".into())),
        Container::ThreeMf => {
            let mut zip = zip::ZipArchive::new(File::open(path)?)?;
            let wanted = plate.unwrap_or_else(|| {
                (0..zip.len())
                    .filter_map(|i| zip.by_index(i).ok().map(|e| e.name().to_string()))
                    .filter_map(|n| plate_number(&n))
                    .min()
                    .unwrap_or(1)
            });

            let entry_name = (0..zip.len())
                .filter_map(|i| zip.by_index(i).ok().map(|e| e.name().to_string()))
                .find(|n| plate_number(n) == Some(wanted))
                .ok_or(Error::NoGcodeInArchive)?;

            let mut entry = zip.by_name(&entry_name)?;
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            let mut cursor = Cursor::new(bytes);
            Ok((f(&mut cursor)?, Some(wanted)))
        }
    }
}

/// `Metadata/plate_3.gcode` -> `Some(3)`. Anything else -> `None`.
fn plate_number(entry: &str) -> Option<u32> {
    let lower = entry.to_ascii_lowercase();
    if !lower.ends_with(".gcode") {
        return None;
    }
    let stem = lower.rsplit('/').next()?.strip_suffix(".gcode")?;
    stem.strip_prefix("plate_")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_containers_by_extension() {
        assert_eq!(
            Container::of(Path::new("a/b.gcode")).unwrap(),
            Container::Plain
        );
        assert_eq!(
            Container::of(Path::new("a/b.GCODE")).unwrap(),
            Container::Plain
        );
        assert_eq!(
            Container::of(Path::new("a/b.gcode.3mf")).unwrap(),
            Container::ThreeMf
        );
        assert_eq!(
            Container::of(Path::new("a/b.bgcode")).unwrap(),
            Container::BinaryGcode
        );
        assert!(Container::of(Path::new("a/b.stl")).is_err());
    }

    #[test]
    fn reads_plate_numbers_from_archive_entries() {
        assert_eq!(plate_number("Metadata/plate_1.gcode"), Some(1));
        assert_eq!(plate_number("Metadata/plate_12.gcode"), Some(12));
        assert_eq!(plate_number("Metadata/plate_1.png"), None);
        assert_eq!(plate_number("3D/3dmodel.model"), None);
    }
}
