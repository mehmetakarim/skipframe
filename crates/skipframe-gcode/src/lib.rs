//! SkipFrame's G-code front end.
//!
//! Three layers, deliberately separable:
//!
//! * [`container`] -- what kind of file this is (`.gcode`, `.gcode.3mf`, one day `.bgcode`)
//! * [`parse`] layer 1 -- motion. Dialect independent, always correct, always enough for a video
//! * [`parse`] layer 2 + [`dialect`] + [`profile`] -- layer boundaries, feature types, widths,
//!   printer identity. Best effort; degrades with a warning instead of failing
//!
//! The output is always an [`ir::Ir`], encoded with [`ir::Ir::encode`] into the flat little
//! endian buffer that crosses the IPC boundary and lands in the parse cache unchanged.

pub mod container;
pub mod dialect;
pub mod error;
pub mod ir;
pub mod parse;
pub mod profile;

use std::io::Read;
use std::path::Path;

pub use error::{Error, Result, Warning};
pub use ir::{FeatureType, Ir, Meta};
pub use parse::ParseOptions;

/// Parse any supported container into the IR.
///
/// `plate` selects a plate inside a `.gcode.3mf`; it is ignored for plain files. When `None`,
/// the lowest-numbered plate is used.
pub fn parse_file(path: &Path, plate: Option<u32>) -> Result<Ir> {
    let source_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed.gcode")
        .to_string();
    let opts = ParseOptions { source_name };

    let (mut ir, used_plate) =
        container::with_reader(path, plate, |reader| parse::parse_stream(reader, &opts))?;
    ir.meta.plate = used_plate;
    // The UI shows the file's size next to its layer count, and only the caller that opened the
    // path knows it -- the parser sees a reader.
    ir.meta.source_bytes = std::fs::metadata(path).ok().map(|m| m.len());
    Ok(ir)
}

/// Cache key for a source file: SHA-256 of its bytes, namespaced by the IR format version so a
/// format bump invalidates every entry without any migration code.
pub fn cache_key(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    Ok(format!("v{}-{:x}", ir::VERSION, digest))
}
