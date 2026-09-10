//! SkipFrame intermediate representation: structure of arrays, one flat byte buffer.
//!
//! The buffer produced by [`Ir::encode`] is what crosses the Tauri IPC boundary verbatim
//! (`tauri::ipc::Response::new(Vec<u8>)`) and what the parse cache stores on disk. It is
//! **not** JSON and must never be serialised as such.
//!
//! # Binary layout (`SFIR` v1, little endian)
//!
//! ```text
//! off  size  field
//!   0     4  magic            b"SFIR"
//!   4     2  version          u16 = 1
//!   6     2  flags            u16 (reserved, 0)
//!   8     4  segment_count    u32  -- number of line segments
//!  12     4  layer_count      u32  -- number of layers
//!  16     4  positions_off    u32  -- f32 * segment_count * 6   (x,y,z start, x,y,z end)
//!  20     4  layer_start_off  u32  -- u32 * (layer_count + 1)   segment offsets, sentinel last
//!  24     4  feature_off      u32  -- u8  * segment_count
//!  28     4  tool_off         u32  -- u8  * segment_count
//!  32     4  width_off        u32  -- f32 * segment_count
//!  36     4  meta_off         u32  -- UTF-8 JSON
//!  40     4  meta_len         u32
//!  44     4  total_len        u32
//!  48    16  reserved         zeroes
//!  64   ...  section payloads, each aligned to 8 bytes, in the order listed above
//! ```
//!
//! # Granularity
//!
//! `positions` is **per vertex**: every segment contributes two vertices, so the array can be
//! handed to a `THREE.BufferAttribute` and animated with `setDrawRange(0, n * 2)` without any
//! index buffer or per-layer mesh.
//!
//! `feature_type`, `tool_index` and `width` are **per segment** -- one entry each, not two.
//! A renderer that needs them as vertex attributes expands them once at upload time.
//!
//! `layer_start[i]` is the **segment** offset where layer `i` begins; the array carries a
//! trailing sentinel equal to `segment_count`, so layer `i` covers segments
//! `layer_start[i] .. layer_start[i + 1]`.
//!
//! Coordinates are stored exactly as the G-code states them (millimetres, Z up). The renderer,
//! not the parser, is responsible for converting to Three.js' Y-up convention.

use serde::{Deserialize, Serialize};

pub const MAGIC: [u8; 4] = *b"SFIR";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 64;

/// Extrusion role of a segment. Layer 2 (semantic) output; defaults to
/// [`FeatureType::Unknown`] for extrusions and [`FeatureType::Travel`] for non-extruding
/// moves when the dialect emits no usable hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum FeatureType {
    Travel = 0,
    OuterWall = 1,
    InnerWall = 2,
    SolidInfill = 3,
    SparseInfill = 4,
    Support = 5,
    SkirtBrim = 6,
    Bridge = 7,
    TopSurface = 8,
    Unknown = 9,
}

impl FeatureType {
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Everything that is small, human-readable and not per-segment. Serialised as JSON into the
/// tail of the IR buffer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Detected slicer dialect, e.g. `"PrusaSlicer"`. `"Unknown"` when the header matched none.
    pub dialect: String,
    /// Raw slicer version string from the header comment, when present.
    pub slicer_version: Option<String>,
    /// Printer model as claimed by the file (`printer_model`, Cura `machine_name`, ...).
    pub printer_model: Option<String>,
    /// Resolved printer profile id from our built-in table, if one matched.
    pub printer_profile: Option<String>,
    /// Bed size in millimetres, `[x, y]`, from a profile or from `bed_shape` / `;MAXX`.
    pub bed_size: Option<[f32; 2]>,
    /// Bed origin offset in millimetres -- nonzero for centred-origin machines.
    pub bed_origin: Option<[f32; 2]>,
    /// Slicer's own print time estimate, seconds.
    pub estimated_time_s: Option<f64>,
    /// Filament used, grams.
    pub filament_grams: Option<f64>,
    /// Filament used, millimetres of stock.
    pub filament_mm: Option<f64>,
    pub layer_count: u32,
    pub segment_count: u32,
    /// Z height of each layer, millimetres. Length == `layer_count`.
    pub layer_z: Vec<f32>,
    /// Axis-aligned bounds of extruding moves only, `[minx, miny, minz, maxx, maxy, maxz]`.
    pub bounds: [f32; 6],
    /// Number of distinct tools/extruders seen.
    pub tool_count: u8,
    /// Nozzle diameter in millimetres, when the header declares one.
    pub nozzle_diameter: Option<f32>,
    /// Whether layer 2 produced real feature types or fell back to `Unknown`.
    pub has_feature_types: bool,
    /// Whether extrusion width came from the file rather than being derived from E deltas.
    pub has_declared_width: bool,
    /// Non-fatal degradations, shown to the user without stopping the job.
    pub warnings: Vec<String>,
    /// Source file name (not a full path -- the IR is cached and must not leak the user's tree).
    pub source_name: String,
    /// Size of the source file on disk, bytes. Absent in cache entries written before this
    /// field existed, which is why it is optional rather than a plain number.
    #[serde(default)]
    pub source_bytes: Option<u64>,
    /// For `.gcode.3mf`, the plate that was extracted.
    pub plate: Option<u32>,
}

/// In-memory IR, before encoding. Grown by the parser as it streams.
#[derive(Debug, Default)]
pub struct Ir {
    /// `segment_count * 6` floats.
    pub positions: Vec<f32>,
    /// `layer_count + 1` entries, last one is `segment_count`.
    pub layer_start: Vec<u32>,
    /// `segment_count` entries.
    pub feature_type: Vec<u8>,
    /// `segment_count` entries.
    pub tool_index: Vec<u8>,
    /// `segment_count` entries, millimetres.
    pub width: Vec<f32>,
    pub meta: Meta,
}

impl Ir {
    /// Pre-size the arrays from a byte-length guess so the hot loop reallocates less.
    pub fn with_capacity(segments: usize) -> Self {
        Self {
            positions: Vec::with_capacity(segments * 6),
            layer_start: Vec::with_capacity(1024),
            feature_type: Vec::with_capacity(segments),
            tool_index: Vec::with_capacity(segments),
            width: Vec::with_capacity(segments),
            meta: Meta::default(),
        }
    }

    #[inline]
    pub fn segment_count(&self) -> u32 {
        self.feature_type.len() as u32
    }

    #[inline]
    pub fn push_segment(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
        feature: FeatureType,
        tool: u8,
        width: f32,
    ) {
        self.positions.extend_from_slice(&from);
        self.positions.extend_from_slice(&to);
        self.feature_type.push(feature.as_u8());
        self.tool_index.push(tool);
        self.width.push(width);
    }

    /// Close the IR: append the `layer_start` sentinel and fill in derived meta counters.
    pub fn finish(&mut self) {
        let segs = self.segment_count();
        self.layer_start.push(segs);
        self.meta.segment_count = segs;
        self.meta.layer_count = self.layer_start.len().saturating_sub(1) as u32;
    }

    /// Encode to the wire/cache format described in the module docs.
    pub fn encode(&self) -> Vec<u8> {
        let meta_json = serde_json::to_vec(&self.meta).expect("meta is always serialisable");

        let mut off = HEADER_LEN;
        let positions_off = align8(&mut off, self.positions.len() * 4);
        let layer_start_off = align8(&mut off, self.layer_start.len() * 4);
        let feature_off = align8(&mut off, self.feature_type.len());
        let tool_off = align8(&mut off, self.tool_index.len());
        let width_off = align8(&mut off, self.width.len() * 4);
        let meta_off = align8(&mut off, meta_json.len());
        let total_len = off;

        let mut buf = vec![0u8; total_len];
        buf[0..4].copy_from_slice(&MAGIC);
        buf[4..6].copy_from_slice(&VERSION.to_le_bytes());
        buf[6..8].copy_from_slice(&0u16.to_le_bytes());
        put_u32(&mut buf, 8, self.segment_count());
        put_u32(&mut buf, 12, self.meta.layer_count);
        put_u32(&mut buf, 16, positions_off as u32);
        put_u32(&mut buf, 20, layer_start_off as u32);
        put_u32(&mut buf, 24, feature_off as u32);
        put_u32(&mut buf, 28, tool_off as u32);
        put_u32(&mut buf, 32, width_off as u32);
        put_u32(&mut buf, 36, meta_off as u32);
        put_u32(&mut buf, 40, meta_json.len() as u32);
        put_u32(&mut buf, 44, total_len as u32);

        write_f32(&mut buf, positions_off, &self.positions);
        write_u32(&mut buf, layer_start_off, &self.layer_start);
        buf[feature_off..feature_off + self.feature_type.len()].copy_from_slice(&self.feature_type);
        buf[tool_off..tool_off + self.tool_index.len()].copy_from_slice(&self.tool_index);
        write_f32(&mut buf, width_off, &self.width);
        buf[meta_off..meta_off + meta_json.len()].copy_from_slice(&meta_json);

        buf
    }
}

/// Replace the file-identity fields inside an already-encoded buffer.
///
/// The parse cache is keyed by the content's hash, so the same bytes under two names share one
/// entry — which is what makes reopening a renamed file free. But the name and the size on disk
/// are properties of the path, not of the content, and a cache hit must not hand back the name
/// of whichever copy happened to be parsed first: the exported video would be named after the
/// wrong file.
///
/// The meta JSON is the last section of the buffer, so it can be replaced by truncating and
/// appending; only its length and the total length need fixing up.
pub fn retag(buf: &mut Vec<u8>, source_name: &str, source_bytes: Option<u64>) -> bool {
    if buf.len() < HEADER_LEN || buf[0..4] != MAGIC {
        return false;
    }
    let meta_off = u32::from_le_bytes(buf[36..40].try_into().unwrap()) as usize;
    let meta_len = u32::from_le_bytes(buf[40..44].try_into().unwrap()) as usize;
    if meta_off + meta_len > buf.len() {
        return false;
    }

    let Ok(mut meta) = serde_json::from_slice::<Meta>(&buf[meta_off..meta_off + meta_len]) else {
        return false;
    };
    if meta.source_name == source_name && meta.source_bytes == source_bytes {
        return true;
    }
    meta.source_name = source_name.to_string();
    meta.source_bytes = source_bytes;

    let Ok(json) = serde_json::to_vec(&meta) else {
        return false;
    };
    buf.truncate(meta_off);
    buf.extend_from_slice(&json);
    put_u32(buf, 40, json.len() as u32);
    let total = buf.len() as u32;
    put_u32(buf, 44, total);
    true
}

/// Reserve `len` bytes at the next 8-byte boundary; returns the offset and advances the cursor.
fn align8(cursor: &mut usize, len: usize) -> usize {
    let start = (*cursor + 7) & !7;
    *cursor = start + len;
    start
}

#[inline]
fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn write_f32(buf: &mut [u8], at: usize, src: &[f32]) {
    for (i, v) in src.iter().enumerate() {
        buf[at + i * 4..at + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn write_u32(buf: &mut [u8], at: usize, src: &[u32]) {
    for (i, v) in src.iter().enumerate() {
        buf[at + i * 4..at + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_readable_header() {
        let mut ir = Ir::default();
        ir.layer_start.push(0);
        ir.push_segment([0.0; 3], [1.0, 0.0, 0.2], FeatureType::OuterWall, 0, 0.45);
        ir.finish();
        let buf = ir.encode();

        assert_eq!(&buf[0..4], &MAGIC);
        assert_eq!(u16::from_le_bytes([buf[4], buf[5]]), VERSION);
        assert_eq!(u32::from_le_bytes(buf[8..12].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(buf[12..16].try_into().unwrap()), 1);
        assert_eq!(
            buf.len(),
            u32::from_le_bytes(buf[44..48].try_into().unwrap()) as usize
        );
    }

    #[test]
    fn retag_replaces_the_name_and_size_without_disturbing_the_arrays() {
        let mut ir = Ir::default();
        ir.layer_start.push(0);
        for i in 0..7 {
            ir.push_segment(
                [i as f32; 3],
                [i as f32 + 1.0; 3],
                FeatureType::OuterWall,
                0,
                0.45,
            );
        }
        ir.meta.source_name = "first-name.gcode".into();
        ir.meta.source_bytes = Some(111);
        ir.finish();
        let mut buf = ir.encode();
        let positions_before = buf[64..64 + 7 * 6 * 4].to_vec();

        assert!(retag(&mut buf, "renamed-much-longer.gcode", Some(222)));

        // The header still describes the buffer.
        assert_eq!(
            buf.len(),
            u32::from_le_bytes(buf[44..48].try_into().unwrap()) as usize
        );
        assert_eq!(u32::from_le_bytes(buf[8..12].try_into().unwrap()), 7);
        // The arrays are untouched.
        assert_eq!(&buf[64..64 + 7 * 6 * 4], &positions_before[..]);

        let meta_off = u32::from_le_bytes(buf[36..40].try_into().unwrap()) as usize;
        let meta_len = u32::from_le_bytes(buf[40..44].try_into().unwrap()) as usize;
        let meta: Meta = serde_json::from_slice(&buf[meta_off..meta_off + meta_len]).unwrap();
        assert_eq!(meta.source_name, "renamed-much-longer.gcode");
        assert_eq!(meta.source_bytes, Some(222));
        assert_eq!(meta.segment_count, 7);
    }

    #[test]
    fn every_section_is_eight_byte_aligned() {
        let mut ir = Ir::default();
        ir.layer_start.push(0);
        for i in 0..5 {
            ir.push_segment(
                [i as f32; 3],
                [i as f32 + 1.0; 3],
                FeatureType::Travel,
                0,
                0.0,
            );
        }
        ir.finish();
        let buf = ir.encode();
        for at in [16usize, 20, 24, 28, 32, 36] {
            let off = u32::from_le_bytes(buf[at..at + 4].try_into().unwrap()) as usize;
            assert_eq!(off % 8, 0, "section at header offset {at} is misaligned");
        }
    }
}
