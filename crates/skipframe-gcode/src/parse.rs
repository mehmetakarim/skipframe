//! Layer 1 (geometry) and layer 2 (semantics) in one streaming pass.
//!
//! Layer 1 is unconditional and dialect-independent: `G0/G1/G2/G3`, `X/Y/Z/E/F`, `G90/G91`,
//! `M82/M83`, `G92`, `G20/G21`, `T<n>`. A video can always be produced from layer 1 alone.
//!
//! Layer 2 reads comments -- layer changes, feature types, widths, printer configuration. Every
//! part of it can fail independently; each failure records a [`Warning`] and the pass continues.

use std::io::BufRead;

use crate::dialect::Dialect;
use crate::error::{Error, Result, Warning};
use crate::ir::{FeatureType, Ir};
use crate::profile;

/// Maximum chord deviation when flattening `G2`/`G3` arcs, millimetres.
const ARC_TOLERANCE_MM: f32 = 0.02;
const DEFAULT_FILAMENT_DIAMETER: f32 = 1.75;
const DEFAULT_LAYER_HEIGHT: f32 = 0.2;

/// Sentinel bounding box: min at +inf, max at -inf, so the first point sets both.
const EMPTY_BOUNDS: [f32; 6] = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// File name shown in the UI and stored in meta. Never a full path.
    pub source_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerSource {
    /// Nothing decided yet.
    Undetermined,
    /// The file told us where layers start.
    Markers,
    /// We inferred layer boundaries from Z movement.
    ZInferred,
}

/// Parse a plain G-code stream into the IR. Returns [`Error::NoMoves`] when the file contains
/// no printable movement at all.
pub fn parse_stream(reader: &mut dyn BufRead, opts: &ParseOptions) -> Result<Ir> {
    let mut p = Parser::new(opts);
    let mut line: Vec<u8> = Vec::with_capacity(256);

    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let mut l: &[u8] = &line;
        while let Some(&c) = l.last() {
            if c == b'\n' || c == b'\r' {
                l = &l[..l.len() - 1];
            } else {
                break;
            }
        }
        match memchr::memchr(b';', l) {
            Some(i) => {
                if i > 0 {
                    p.handle_code(&l[..i]);
                }
                p.handle_comment(&l[i + 1..]);
            }
            None => p.handle_code(l),
        }
    }

    p.finish()
}

struct Parser {
    ir: Ir,
    dialect: Dialect,
    slicer_version: Option<String>,

    // --- machine state -------------------------------------------------------------------
    pos: [f32; 3],
    /// Millimetres per G-code unit; 25.4 after `G20`.
    unit_scale: f32,
    absolute_xyz: bool,
    absolute_e: bool,
    e_current: f32,
    tool: u8,
    max_tool: u8,

    // --- layer 2 state -------------------------------------------------------------------
    feature: FeatureType,
    declared_width: Option<f32>,
    layer_source: LayerSource,
    /// Z of the layer currently being filled; used by the Z-inference fallback.
    current_layer_z: f32,
    /// The current layer's Z has not yet been confirmed by a real extruding move.
    layer_z_pending: bool,
    layer_height: f32,
    filament_area: f32,

    // --- meta accumulators ---------------------------------------------------------------
    printer_model: Option<String>,
    bed_size: Option<[f32; 2]>,
    bed_origin: Option<[f32; 2]>,
    print_max: [f32; 2],
    estimated_time_s: Option<f64>,
    filament_grams: Option<f64>,
    filament_mm: Option<f64>,
    nozzle_diameter: Option<f32>,
    bounds: [f32; 6],
    saw_feature_marker: bool,
    saw_declared_width: bool,
    warnings: Vec<Warning>,
    source_name: String,
}

impl Parser {
    fn new(opts: &ParseOptions) -> Self {
        Self {
            ir: Ir::with_capacity(1 << 16),
            dialect: Dialect::Unknown,
            slicer_version: None,
            pos: [0.0; 3],
            unit_scale: 1.0,
            absolute_xyz: true,
            absolute_e: true,
            e_current: 0.0,
            tool: 0,
            max_tool: 0,
            feature: FeatureType::Unknown,
            declared_width: None,
            layer_source: LayerSource::Undetermined,
            current_layer_z: f32::NAN,
            layer_z_pending: false,
            layer_height: DEFAULT_LAYER_HEIGHT,
            filament_area: filament_area(DEFAULT_FILAMENT_DIAMETER),
            printer_model: None,
            bed_size: None,
            bed_origin: None,
            print_max: [0.0, 0.0],
            estimated_time_s: None,
            filament_grams: None,
            filament_mm: None,
            nozzle_diameter: None,
            bounds: EMPTY_BOUNDS,
            saw_feature_marker: false,
            saw_declared_width: false,
            warnings: Vec::new(),
            source_name: opts.source_name.clone(),
        }
    }

    // -- layer 1 ------------------------------------------------------------------------

    fn handle_code(&mut self, code: &[u8]) {
        let mut fields = Fields::new(code);
        let Some((letter, number)) = fields.next() else {
            return;
        };

        match letter {
            b'G' => match number as i32 {
                0 | 1 => self.linear_move(&mut fields),
                2 | 3 => self.arc_move(&mut fields, number as i32 == 2),
                20 => self.unit_scale = 25.4,
                21 => self.unit_scale = 1.0,
                28 => self.home(&mut fields),
                90 => self.absolute_xyz = true,
                91 => self.absolute_xyz = false,
                92 => self.set_position(&mut fields),
                _ => {}
            },
            b'M' => match number as i32 {
                82 => self.absolute_e = true,
                83 => self.absolute_e = false,
                _ => {}
            },
            b'T' => {
                let t = number as i32;
                if (0..=254).contains(&t) {
                    self.tool = t as u8;
                    self.max_tool = self.max_tool.max(self.tool);
                }
            }
            _ => {}
        }
    }

    fn linear_move(&mut self, fields: &mut Fields<'_>) {
        let mut to = self.pos;
        let mut e_delta = 0.0f32;

        for (letter, value) in fields.by_ref() {
            let v = value * self.unit_scale;
            match letter {
                b'X' => {
                    to[0] = if self.absolute_xyz {
                        v
                    } else {
                        self.pos[0] + v
                    }
                }
                b'Y' => {
                    to[1] = if self.absolute_xyz {
                        v
                    } else {
                        self.pos[1] + v
                    }
                }
                b'Z' => {
                    to[2] = if self.absolute_xyz {
                        v
                    } else {
                        self.pos[2] + v
                    }
                }
                b'E' => {
                    // E is nominally scaled by G20/G21 too, but no slicer mixes inches with
                    // extrusion, and treating it as raw keeps the width maths honest.
                    if self.absolute_e {
                        e_delta = value - self.e_current;
                        self.e_current = value;
                    } else {
                        e_delta = value;
                        self.e_current += value;
                    }
                }
                _ => {}
            }
        }
        self.emit(to, e_delta);
    }

    /// `G2`/`G3` with `I`/`J` centre offsets (or `R` radius), flattened to chords.
    fn arc_move(&mut self, fields: &mut Fields<'_>, clockwise: bool) {
        let from = self.pos;
        let mut to = self.pos;
        let mut ij: Option<[f32; 2]> = None;
        let mut radius: Option<f32> = None;
        let mut e_delta = 0.0f32;

        let mut i_off = 0.0f32;
        let mut j_off = 0.0f32;
        let mut saw_ij = false;

        for (letter, value) in fields.by_ref() {
            let v = value * self.unit_scale;
            match letter {
                b'X' => to[0] = if self.absolute_xyz { v } else { from[0] + v },
                b'Y' => to[1] = if self.absolute_xyz { v } else { from[1] + v },
                b'Z' => to[2] = if self.absolute_xyz { v } else { from[2] + v },
                b'I' => {
                    i_off = v;
                    saw_ij = true;
                }
                b'J' => {
                    j_off = v;
                    saw_ij = true;
                }
                b'R' => radius = Some(v),
                b'E' => {
                    if self.absolute_e {
                        e_delta = value - self.e_current;
                        self.e_current = value;
                    } else {
                        e_delta = value;
                        self.e_current += value;
                    }
                }
                _ => {}
            }
        }
        if saw_ij {
            ij = Some([i_off, j_off]);
        }

        let centre = match (ij, radius) {
            (Some([i, j]), _) => [from[0] + i, from[1] + j],
            (None, Some(r)) => match centre_from_radius(from, to, r, clockwise) {
                Some(c) => c,
                None => {
                    self.warn(Warning::UnsupportedArcWithoutOffsets);
                    return self.emit(to, e_delta);
                }
            },
            (None, None) => {
                self.warn(Warning::UnsupportedArcWithoutOffsets);
                return self.emit(to, e_delta);
            }
        };

        let steps = arc_steps(from, to, centre, clockwise);
        if steps <= 1 {
            return self.emit(to, e_delta);
        }

        let a0 = (from[1] - centre[1]).atan2(from[0] - centre[0]);
        let sweep = arc_sweep(from, to, centre, clockwise);
        let r = ((from[0] - centre[0]).powi(2) + (from[1] - centre[1]).powi(2)).sqrt();
        let e_step = e_delta / steps as f32;

        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let a = a0 + sweep * t;
            let p = [
                centre[0] + r * a.cos(),
                centre[1] + r * a.sin(),
                from[2] + (to[2] - from[2]) * t,
            ];
            let p = if s == steps { to } else { p };
            self.emit(p, e_step);
        }
    }

    fn home(&mut self, fields: &mut Fields<'_>) {
        let mut any = false;
        for (letter, _) in fields.by_ref() {
            match letter {
                b'X' => {
                    self.pos[0] = 0.0;
                    any = true;
                }
                b'Y' => {
                    self.pos[1] = 0.0;
                    any = true;
                }
                b'Z' => {
                    self.pos[2] = 0.0;
                    any = true;
                }
                _ => {}
            }
        }
        if !any {
            self.pos = [0.0; 3];
        }
    }

    fn set_position(&mut self, fields: &mut Fields<'_>) {
        for (letter, value) in fields.by_ref() {
            let v = value * self.unit_scale;
            match letter {
                b'X' => self.pos[0] = v,
                b'Y' => self.pos[1] = v,
                b'Z' => self.pos[2] = v,
                b'E' => self.e_current = value,
                _ => {}
            }
        }
    }

    /// Turn a resolved destination into a segment (or into nothing, for retractions).
    fn emit(&mut self, to: [f32; 3], e_delta: f32) {
        let from = self.pos;
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let dz = to[2] - from[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        // A pure retract/prime or a duplicate coordinate is machine state, not geometry.
        if dist < 1e-6 {
            self.pos = to;
            return;
        }

        let extruding = e_delta > 1e-9;

        if extruding {
            self.begin_layer_if_needed(to[2]);
            if self.layer_z_pending {
                if let Some(last) = self.ir.meta.layer_z.last_mut() {
                    *last = to[2];
                }
                self.current_layer_z = to[2];
                self.layer_z_pending = false;
            }
            self.grow_bounds(from);
            self.grow_bounds(to);
        } else if self.ir.layer_start.is_empty() {
            // Travel before the first layer still needs somewhere to live.
            self.ir.layer_start.push(0);
            self.ir.meta.layer_z.push(to[2]);
            self.layer_z_pending = true;
        }

        let (feature, width) = if extruding {
            let w = match self.declared_width {
                Some(w) => w,
                None => derive_width(e_delta, dist, self.layer_height, self.filament_area),
            };
            let f = if self.feature == FeatureType::Travel {
                FeatureType::Unknown
            } else {
                self.feature
            };
            (f, w)
        } else {
            (FeatureType::Travel, 0.0)
        };

        self.ir.push_segment(from, to, feature, self.tool, width);
        self.pos = to;
    }

    fn grow_bounds(&mut self, p: [f32; 3]) {
        for (axis, v) in p.iter().enumerate() {
            if *v < self.bounds[axis] {
                self.bounds[axis] = *v;
            }
            if *v > self.bounds[axis + 3] {
                self.bounds[axis + 3] = *v;
            }
        }
        self.print_max[0] = self.print_max[0].max(p[0]);
        self.print_max[1] = self.print_max[1].max(p[1]);
    }

    // -- layer 2 ------------------------------------------------------------------------

    fn handle_comment(&mut self, raw: &[u8]) {
        let Ok(s) = std::str::from_utf8(raw) else {
            return;
        };
        let c = s.trim();
        if c.is_empty() {
            return;
        }

        if self.dialect == Dialect::Unknown {
            if let Some((d, version)) = Dialect::detect(c) {
                self.dialect = d;
                self.slicer_version = version;
                return;
            }
        }

        if self.dialect.layer_marker(c).is_some() {
            if self.layer_source != LayerSource::Markers {
                // Everything emitted before the first layer marker is start G-code: priming, a
                // purge line, a wipe. It is real geometry and it stays in the IR, but it is not
                // a layer of the model. Folding it into the first marked layer is what keeps our
                // layer count equal to the one the slicer printed in its own header.
                self.layer_source = LayerSource::Markers;
                self.ir.layer_start.clear();
                self.ir.meta.layer_z.clear();
                // The first marked layer owns the start block, so it begins at segment zero.
                self.ir.layer_start.push(0);
                self.ir.meta.layer_z.push(self.pos[2]);
                self.layer_z_pending = true;
                // A purge line runs the full width of the plate at the very front. It is real
                // extruded material and it stays in the IR, but letting it define the bounding
                // box would misreport the model's size and aim the camera at the bed edge.
                self.bounds = EMPTY_BOUNDS;
                return;
            }
            self.start_layer(None);
            return;
        }
        if let Some(f) = self.dialect.feature_marker(c) {
            self.saw_feature_marker = true;
            self.feature = f;
            return;
        }
        if let Some(w) = self.dialect.width_marker(c) {
            if w > 0.0 {
                self.saw_declared_width = true;
                self.declared_width = Some(w);
            }
            return;
        }
        if self.layer_z_pending {
            if let Some(z) = self.dialect.layer_z(c) {
                if let Some(last) = self.ir.meta.layer_z.last_mut() {
                    *last = z;
                }
                self.update_layer_height(z);
                self.current_layer_z = z;
                self.layer_z_pending = false;
                return;
            }
        }

        self.handle_config(c);
    }

    /// Start a new layer at the current segment offset. Called by markers and by Z inference.
    fn start_layer(&mut self, z: Option<f32>) {
        let seg = self.ir.segment_count();
        if self.ir.layer_start.last() == Some(&seg) && !self.ir.layer_start.is_empty() {
            // Two markers with nothing between them -- keep one layer, refresh its Z.
            if let (Some(z), Some(last)) = (z, self.ir.meta.layer_z.last_mut()) {
                *last = z;
            }
            self.layer_z_pending = z.is_none();
            return;
        }
        self.ir.layer_start.push(seg);
        self.ir.meta.layer_z.push(z.unwrap_or(self.pos[2]));
        if let Some(z) = z {
            self.update_layer_height(z);
            self.current_layer_z = z;
        }
        self.layer_z_pending = z.is_none();
    }

    fn begin_layer_if_needed(&mut self, z: f32) {
        if self.ir.layer_start.is_empty() {
            self.start_layer(Some(z));
            return;
        }
        if self.layer_source != LayerSource::Markers
            && self.current_layer_z.is_finite()
            && (z - self.current_layer_z).abs() > 1e-4
        {
            self.layer_source = LayerSource::ZInferred;
            self.start_layer(Some(z));
        }
    }

    fn update_layer_height(&mut self, z: f32) {
        if self.current_layer_z.is_finite() {
            let h = z - self.current_layer_z;
            if h > 1e-4 && h < 2.0 {
                self.layer_height = h;
            }
        }
    }

    /// `key = value` (PrusaSlicer family) and `KEY:value` (Cura family) configuration comments.
    fn handle_config(&mut self, c: &str) {
        let (key, value) = match c.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => match c.split_once(':') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => return,
            },
        };
        if value.is_empty() {
            return;
        }
        let k = key.to_ascii_lowercase();

        match k.as_str() {
            "printer_model" | "machine_name" | "target_machine.name" | "printer_settings_id" => {
                if self.printer_model.is_none() {
                    self.printer_model = Some(value.to_string());
                }
            }
            "bed_shape" | "printable_area" => {
                if let Some((size, origin)) = profile::from_bed_shape(value) {
                    self.bed_size = Some(size);
                    self.bed_origin = Some(origin);
                }
            }
            "layer_height" => {
                if let Ok(v) = value.parse::<f32>() {
                    if v > 0.0 {
                        self.layer_height = v;
                    }
                }
            }
            "nozzle_diameter" => {
                if let Ok(v) = first_number(value) {
                    self.nozzle_diameter = Some(v);
                }
            }
            "filament_diameter" => {
                if let Ok(v) = first_number(value) {
                    if v > 0.1 {
                        self.filament_area = filament_area(v);
                    }
                }
            }
            "maxx" => self.print_max[0] = self.print_max[0].max(value.parse().unwrap_or(0.0)),
            "maxy" => self.print_max[1] = self.print_max[1].max(value.parse().unwrap_or(0.0)),
            "time" => self.estimated_time_s = value.parse::<f64>().ok(),
            _ => {
                if k.starts_with("filament used [g]") || k.starts_with("total filament weight") {
                    self.filament_grams = first_number(value).ok().map(|v| v as f64);
                } else if k.starts_with("filament used [mm]") {
                    self.filament_mm = first_number(value).ok().map(|v| v as f64);
                } else if k.starts_with("filament used") {
                    // Cura: ";Filament used: 1.234m"
                    if let Ok(v) = first_number(value) {
                        self.filament_mm = Some(v as f64 * 1000.0);
                    }
                } else if k.contains("estimated") && k.contains("time") {
                    // Orca and PrusaSlicer follow the total with
                    // "estimated first layer printing time", which must not overwrite it.
                    if !k.contains("first layer") {
                        self.estimated_time_s = parse_duration(value);
                    }
                }
            }
        }
    }

    fn warn(&mut self, w: Warning) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    fn finish(mut self) -> Result<Ir> {
        if self.ir.segment_count() == 0 {
            return Err(Error::NoMoves);
        }

        if self.dialect == Dialect::Unknown {
            self.warn(Warning::UnknownDialect);
        }
        if self.layer_source != LayerSource::Markers {
            self.warn(Warning::NoLayerMarkers);
        }
        if !self.saw_feature_marker {
            self.warn(Warning::NoFeatureMarkers);
        }
        if !self.saw_declared_width {
            self.warn(Warning::DerivedWidth);
        }

        let matched = self.printer_model.as_deref().and_then(profile::lookup);
        if matched.is_none() && self.bed_size.is_none() {
            self.warn(Warning::NoPrinterProfile);
        }

        let bed = self.bed_size.or(matched.map(|m| m.bed)).unwrap_or_else(|| {
            if self.print_max[0] > 0.0 {
                profile::from_print_bounds(self.print_max[0], self.print_max[1])
            } else {
                profile::GENERIC_BED
            }
        });

        if !self.bounds[0].is_finite() || self.bounds[0] > self.bounds[3] {
            self.bounds = [0.0; 6];
        }

        self.ir.meta = crate::ir::Meta {
            dialect: self.dialect.name().to_string(),
            slicer_version: self.slicer_version.clone(),
            printer_model: self.printer_model.clone(),
            printer_profile: matched.map(|m| m.id.to_string()),
            bed_size: Some(bed),
            bed_origin: self.bed_origin.or(matched.map(|m| m.origin)),
            estimated_time_s: self.estimated_time_s,
            filament_grams: self.filament_grams,
            filament_mm: self.filament_mm,
            layer_count: 0,
            segment_count: 0,
            layer_z: std::mem::take(&mut self.ir.meta.layer_z),
            bounds: self.bounds,
            tool_count: self.max_tool + 1,
            nozzle_diameter: self.nozzle_diameter,
            has_feature_types: self.saw_feature_marker,
            has_declared_width: self.saw_declared_width,
            warnings: self.warnings.iter().map(|w| w.to_string()).collect(),
            source_name: std::mem::take(&mut self.source_name),
            source_bytes: None,
            plate: None,
        };

        self.ir.finish();
        Ok(self.ir)
    }
}

// -- helpers ----------------------------------------------------------------------------

#[inline]
fn filament_area(diameter_mm: f32) -> f32 {
    std::f32::consts::PI * (diameter_mm * 0.5) * (diameter_mm * 0.5)
}

/// Extrusion width from filament consumed, for slicers that do not declare `;WIDTH:`.
/// Volume of filament pushed == width * layer height * travelled distance.
#[inline]
fn derive_width(e_delta_mm: f32, dist_mm: f32, layer_h: f32, filament_area: f32) -> f32 {
    if dist_mm <= 0.0 || layer_h <= 0.0 {
        return 0.0;
    }
    let w = (e_delta_mm * filament_area) / (dist_mm * layer_h);
    // Clamp to something physically plausible so a bad layer-height guess cannot poison the IR.
    w.clamp(0.0, 5.0)
}

fn arc_sweep(from: [f32; 3], to: [f32; 3], centre: [f32; 2], clockwise: bool) -> f32 {
    let a0 = (from[1] - centre[1]).atan2(from[0] - centre[0]);
    let a1 = (to[1] - centre[1]).atan2(to[0] - centre[0]);
    let mut sweep = a1 - a0;
    let tau = std::f32::consts::TAU;
    if clockwise {
        while sweep > 0.0 {
            sweep -= tau;
        }
        if sweep <= -tau {
            sweep += tau;
        }
    } else {
        while sweep < 0.0 {
            sweep += tau;
        }
        if sweep >= tau {
            sweep -= tau;
        }
    }
    // A full circle is encoded as start == end; treat a zero sweep as one full turn.
    if sweep.abs() < 1e-6 {
        if clockwise {
            -tau
        } else {
            tau
        }
    } else {
        sweep
    }
}

fn arc_steps(from: [f32; 3], to: [f32; 3], centre: [f32; 2], clockwise: bool) -> u32 {
    let r = ((from[0] - centre[0]).powi(2) + (from[1] - centre[1]).powi(2)).sqrt();
    if !r.is_finite() || r <= ARC_TOLERANCE_MM {
        return 1;
    }
    let sweep = arc_sweep(from, to, centre, clockwise).abs();
    // Chord deviation d for step angle a is r * (1 - cos(a/2)); solve for a.
    let ratio = (1.0 - ARC_TOLERANCE_MM / r).clamp(-1.0, 1.0);
    let step_angle = 2.0 * ratio.acos();
    if step_angle <= 1e-5 {
        return 1;
    }
    ((sweep / step_angle).ceil() as u32).clamp(1, 512)
}

/// Marlin's `R` form: the centre lies on the perpendicular bisector of the chord.
fn centre_from_radius(from: [f32; 3], to: [f32; 3], r: f32, clockwise: bool) -> Option<[f32; 2]> {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1e-6 || d > 2.0 * r.abs() {
        return None;
    }
    let h = ((r * r) - (d * d * 0.25)).max(0.0).sqrt();
    let mx = (from[0] + to[0]) * 0.5;
    let my = (from[1] + to[1]) * 0.5;
    // Sign convention: positive R takes the short way round.
    let sign = if (r > 0.0) == clockwise { -1.0 } else { 1.0 };
    Some([mx + sign * h * (-dy / d), my + sign * h * (dx / d)])
}

fn first_number(value: &str) -> std::result::Result<f32, ()> {
    let token: String = value
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    token.parse::<f32>().map_err(|_| ())
}

/// `"1h 2m 3s"`, `"2d 4h"`, `"1234"` -> seconds.
fn parse_duration(value: &str) -> Option<f64> {
    if let Ok(v) = value.trim().parse::<f64>() {
        return Some(v);
    }
    let mut total = 0.0f64;
    let mut number = String::new();
    let mut any = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
        } else {
            let mult = match ch.to_ascii_lowercase() {
                'd' => 86400.0,
                'h' => 3600.0,
                'm' => 60.0,
                's' => 1.0,
                _ => {
                    number.clear();
                    continue;
                }
            };
            if let Ok(v) = number.parse::<f64>() {
                total += v * mult;
                any = true;
            }
            number.clear();
        }
    }
    any.then_some(total)
}

/// Iterator over `letter + number` fields in one G-code command, skipping whitespace.
struct Fields<'a> {
    bytes: &'a [u8],
    i: usize,
}

impl<'a> Fields<'a> {
    #[inline]
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, i: 0 }
    }
}

impl Iterator for Fields<'_> {
    type Item = (u8, f32);

    #[inline]
    fn next(&mut self) -> Option<(u8, f32)> {
        let b = self.bytes;
        while self.i < b.len() && (b[self.i] == b' ' || b[self.i] == b'\t') {
            self.i += 1;
        }
        if self.i >= b.len() {
            return None;
        }
        let letter = b[self.i].to_ascii_uppercase();
        if !letter.is_ascii_alphabetic() {
            self.i += 1;
            return self.next();
        }
        self.i += 1;
        let start = self.i;
        while self.i < b.len() && b[self.i] != b' ' && b[self.i] != b'\t' {
            self.i += 1;
        }
        Some((letter, parse_f32(&b[start..self.i]).unwrap_or(0.0)))
    }
}

const POW10: [f32; 10] = [
    1.0,
    10.0,
    100.0,
    1_000.0,
    10_000.0,
    100_000.0,
    1_000_000.0,
    1e7,
    1e8,
    1e9,
];

/// Fast decimal parser for the short fixed-point numbers G-code actually contains.
/// Anything unusual (exponents, very long mantissas) falls back to the standard parser.
#[inline]
fn parse_f32(bytes: &[u8]) -> Option<f32> {
    let mut i = 0usize;
    let mut neg = false;
    if let Some(&c) = bytes.first() {
        if c == b'-' || c == b'+' {
            neg = c == b'-';
            i = 1;
        }
    }
    let mut mantissa: u64 = 0;
    let mut int_digits = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        mantissa = mantissa * 10 + (bytes[i] - b'0') as u64;
        i += 1;
        int_digits += 1;
        if int_digits > 15 {
            return slow_parse(bytes);
        }
    }
    let mut frac_digits = 0usize;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            mantissa = mantissa * 10 + (bytes[i] - b'0') as u64;
            i += 1;
            frac_digits += 1;
            if frac_digits > 9 {
                return slow_parse(bytes);
            }
        }
    }
    if i != bytes.len() || (int_digits == 0 && frac_digits == 0) {
        return slow_parse(bytes);
    }
    let v = mantissa as f32 / POW10[frac_digits];
    Some(if neg { -v } else { v })
}

#[cold]
fn slow_parse(bytes: &[u8]) -> Option<f32> {
    std::str::from_utf8(bytes).ok()?.trim().parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(src: &str) -> Ir {
        let mut cur = Cursor::new(src.as_bytes().to_vec());
        parse_stream(
            &mut cur,
            &ParseOptions {
                source_name: "test.gcode".into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn fast_number_parser_matches_std() {
        for s in [
            "0", "1", "-1", "12.345", "-0.001", "100.0", "+2.5", ".5", "-.25",
        ] {
            let fast = parse_f32(s.as_bytes()).unwrap();
            let std: f32 = s.parse().unwrap();
            assert!((fast - std).abs() < 1e-6, "{s}: {fast} vs {std}");
        }
        assert!((parse_f32(b"1e3").unwrap() - 1000.0).abs() < 1e-3);
    }

    #[test]
    fn absolute_and_relative_extrusion_agree() {
        let abs = parse("G90\nM82\nG1 X0 Y0 Z0.2\nG1 X10 E1\nG1 X20 E2\n");
        let rel = parse("G90\nM83\nG1 X0 Y0 Z0.2\nG1 X10 E1\nG1 X20 E1\n");
        assert_eq!(abs.segment_count(), rel.segment_count());
        assert_eq!(abs.feature_type, rel.feature_type);
        assert_eq!(abs.positions, rel.positions);
    }

    fn counts(ir: &Ir) -> (usize, usize) {
        let travels = ir
            .feature_type
            .iter()
            .filter(|f| **f == FeatureType::Travel.as_u8())
            .count();
        (ir.feature_type.len() - travels, travels)
    }

    #[test]
    fn g92_resets_the_extruder_without_emitting_geometry() {
        // The opening `G1 Z0.2` is itself a real (travel) move, so three segments in total.
        let ir = parse("G90\nM82\nG1 X0 Y0 Z0.2\nG1 X10 E5\nG92 E0\nG1 X20 E5\n");
        assert_eq!(counts(&ir), (2, 1));
        // The G92 must not turn the following move into a retraction.
        assert_eq!(ir.feature_type[2], FeatureType::Unknown.as_u8());
    }

    #[test]
    fn retractions_and_duplicate_points_emit_nothing() {
        // Lift, one extrusion, then a retract / zero-length move / prime that produce nothing.
        let ir = parse("G90\nM83\nG1 X0 Y0 Z0.2\nG1 X10 E1\nG1 E-0.8\nG1 X10\nG1 E0.8\n");
        assert_eq!(counts(&ir), (1, 1));
    }

    #[test]
    fn travel_moves_are_marked_and_kept() {
        let ir = parse("G90\nM83\nG1 X0 Y0 Z0.2\nG1 X10 Y0 E1\nG1 X50 Y50\nG1 X60 Y50 E1\n");
        assert_eq!(counts(&ir), (2, 2));
        assert_eq!(ir.feature_type[2], FeatureType::Travel.as_u8());
        assert_eq!(ir.width[2], 0.0);
    }

    #[test]
    fn prusa_layers_and_features_are_read() {
        let src = "; generated by PrusaSlicer 2.8.1+win64\n\
                   G90\nM83\n\
                   ;LAYER_CHANGE\n;Z:0.2\n;TYPE:External perimeter\n;WIDTH:0.45\n\
                   G1 X0 Y0 Z0.2\nG1 X10 Y0 E1\n\
                   ;LAYER_CHANGE\n;Z:0.4\n;TYPE:Perimeter\n\
                   G1 X0 Y0 Z0.4\nG1 X10 Y0 E1\n";
        let ir = parse(src);
        assert_eq!(ir.meta.dialect, "PrusaSlicer");
        assert_eq!(ir.meta.layer_count, 2);
        assert_eq!(ir.meta.layer_z, vec![0.2, 0.4]);
        assert!(ir.meta.has_feature_types);
        assert!(ir.meta.has_declared_width);
        assert_eq!(ir.layer_start.last(), Some(&ir.segment_count()));
    }

    #[test]
    fn cura_layers_are_read_and_width_is_derived() {
        let src = ";Generated with Cura_SteamEngine 5.7.0\n\
                   ;Layer height: 0.2\n;MAXX:100\n;MAXY:80\n\
                   G90\nM82\n\
                   ;LAYER:0\n;TYPE:WALL-OUTER\n\
                   G1 X0 Y0 Z0.2\nG1 X10 Y0 E0.33\n\
                   ;LAYER:1\n;TYPE:FILL\n\
                   G1 X0 Y0 Z0.4\nG1 X10 Y0 E0.66\n";
        let ir = parse(src);
        assert_eq!(ir.meta.dialect, "Cura");
        assert_eq!(ir.meta.layer_count, 2);
        assert!(!ir.meta.has_declared_width);
        // 0.33 mm of 1.75 mm filament over 10 mm at 0.2 mm layer height is roughly a 0.4 mm bead.
        let w = ir.width.iter().copied().find(|w| *w > 0.0).unwrap();
        assert!((0.3..0.6).contains(&w), "derived width was {w}");
        assert_eq!(ir.meta.bed_size, Some([120.0, 120.0]));
    }

    #[test]
    fn layers_fall_back_to_z_inference_without_markers() {
        let src = "G90\nM83\nG1 X0 Y0 Z0.2\nG1 X10 E1\nG1 X0 Y0 Z0.4\nG1 X10 E1\n";
        let ir = parse(src);
        assert_eq!(ir.meta.layer_count, 2);
        assert!(ir.meta.warnings.iter().any(|w| w.contains("inferred")));
    }

    #[test]
    fn arcs_are_flattened_into_many_chords() {
        let src = "G90\nM83\nG1 X0 Y0 Z0.2\nG2 X20 Y0 I10 J0 E5\n";
        let ir = parse(src);
        assert!(
            ir.segment_count() > 10,
            "arc produced {} segments",
            ir.segment_count()
        );
        let n = ir.positions.len();
        assert!((ir.positions[n - 3] - 20.0).abs() < 1e-3);
        assert!(ir.positions[n - 2].abs() < 1e-3);
    }

    #[test]
    fn relative_coordinates_are_honoured() {
        let ir = parse("G91\nM83\nG1 X10 Y0 Z0.2 E1\nG1 X10 E1\n");
        let n = ir.positions.len();
        assert!((ir.positions[n - 3] - 20.0).abs() < 1e-3);
    }

    #[test]
    fn inches_are_converted_to_millimetres() {
        let ir = parse("G20\nG90\nM83\nG1 X0 Y0 Z0.1\nG1 X1 E1\n");
        let n = ir.positions.len();
        assert!((ir.positions[n - 3] - 25.4).abs() < 1e-3);
    }

    #[test]
    fn tool_changes_are_recorded() {
        let src = "G90\nM83\nG1 X0 Y0 Z0.2\nG1 X10 E1\nT1\nG1 X20 E1\n";
        let ir = parse(src);
        assert_eq!(ir.tool_index[0], 0);
        assert_eq!(*ir.tool_index.last().unwrap(), 1);
        assert_eq!(ir.meta.tool_count, 2);
    }

    #[test]
    fn a_file_with_no_moves_is_an_error() {
        let mut cur = Cursor::new(b"; nothing here\nM104 S200\n".to_vec());
        assert!(parse_stream(&mut cur, &ParseOptions::default()).is_err());
    }

    #[test]
    fn start_gcode_before_the_first_marker_is_not_a_layer() {
        // Priming and a purge line at a rising Z, exactly as a real slicer's start block does,
        // then two marked layers. The answer must be two layers, not four.
        let src = "; generated by OrcaSlicer 2.4.2
                   G90
M83
G28
                   G1 X0 Y0 Z0.3 F9000
                   G1 X60 Y0 E4 F1200
                   G1 X60 Y0.4 Z0.2 F9000
                   G1 X0 Y0.4 E4 F1200
                   ;LAYER_CHANGE
;Z:0.2
;TYPE:External perimeter
                   G1 X10 Y10 Z0.2 F9000
G1 X20 Y10 E1
                   ;LAYER_CHANGE
;Z:0.4
;TYPE:Perimeter
                   G1 X10 Y10 Z0.4 F9000
G1 X20 Y10 E1
";
        let ir = parse(src);
        assert_eq!(ir.meta.layer_count, 2);
        assert_eq!(ir.meta.layer_z, vec![0.2, 0.4]);
        // The purge line is still in the IR; it just belongs to the first layer.
        assert_eq!(ir.layer_start[0], 0);
        assert_eq!(ir.layer_start.last(), Some(&ir.segment_count()));
        assert!(ir.segment_count() > 4);

        // The purge line spans x 0..60 at y 0; the model is a 10 mm line at x 10..20, y 10.
        // The reported box must describe the model.
        assert_eq!(ir.meta.bounds[0], 10.0, "min x");
        assert_eq!(ir.meta.bounds[3], 20.0, "max x");
        assert_eq!(ir.meta.bounds[1], 10.0, "min y");
    }

    #[test]
    fn the_first_layer_time_does_not_overwrite_the_total() {
        let src = "; generated by OrcaSlicer 2.4.2
                   G90
M83
;LAYER_CHANGE
;Z:0.2
                   G1 X0 Y0 Z0.2
G1 X10 E1
                   ; estimated printing time (normal mode) = 3h 44m 46s
                   ; estimated first layer printing time (normal mode) = 14s
";
        let ir = parse(src);
        assert_eq!(
            ir.meta.estimated_time_s,
            Some(3.0 * 3600.0 + 44.0 * 60.0 + 46.0)
        );
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("1h 2m 3s"), Some(3723.0));
        assert_eq!(parse_duration("2d"), Some(172800.0));
        assert_eq!(parse_duration("1234"), Some(1234.0));
    }
}
