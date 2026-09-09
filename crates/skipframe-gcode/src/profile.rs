//! Printer profiles: turn whatever the header claims into a bed size we can draw.
//!
//! Resolution order, best first:
//!   1. a built-in profile matched on the declared `printer_model`
//!   2. `bed_shape = 0x0,256x0,256x256,0x256` (PrusaSlicer family)
//!   3. Cura's `;MAXX` / `;MAXY` print bounds, rounded up to a plausible bed
//!   4. a generic 220 x 220 bed, with a warning

/// A bed we know the real dimensions of.
#[derive(Debug, Clone, Copy)]
pub struct PrinterProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Bed size in millimetres, `[x, y]`.
    pub bed: [f32; 2],
    /// Origin offset in millimetres for machines that do not put 0,0 at the front-left corner.
    pub origin: [f32; 2],
}

pub const GENERIC_BED: [f32; 2] = [220.0, 220.0];

/// Substring matched against the lower-cased `printer_model`, then the profile it selects.
/// Longer, more specific keys come first: `x1c` must win over `x1`.
const TABLE: &[(&str, PrinterProfile)] = &[
    (
        "x1 carbon",
        p("bambu-x1c", "Bambu Lab X1 Carbon", [256.0, 256.0]),
    ),
    ("x1c", p("bambu-x1c", "Bambu Lab X1 Carbon", [256.0, 256.0])),
    ("x1e", p("bambu-x1e", "Bambu Lab X1E", [256.0, 256.0])),
    ("x1", p("bambu-x1", "Bambu Lab X1", [256.0, 256.0])),
    ("p1s", p("bambu-p1s", "Bambu Lab P1S", [256.0, 256.0])),
    ("p1p", p("bambu-p1p", "Bambu Lab P1P", [256.0, 256.0])),
    (
        "a1 mini",
        p("bambu-a1-mini", "Bambu Lab A1 mini", [180.0, 180.0]),
    ),
    (
        "a1m",
        p("bambu-a1-mini", "Bambu Lab A1 mini", [180.0, 180.0]),
    ),
    ("a1", p("bambu-a1", "Bambu Lab A1", [256.0, 256.0])),
    ("mk4s", p("prusa-mk4s", "Prusa MK4S", [250.0, 210.0])),
    ("mk4", p("prusa-mk4", "Prusa MK4", [250.0, 210.0])),
    ("mk3.9", p("prusa-mk39", "Prusa MK3.9", [250.0, 210.0])),
    ("mk3", p("prusa-mk3", "Prusa MK3S+", [250.0, 210.0])),
    ("mini", p("prusa-mini", "Prusa MINI+", [180.0, 180.0])),
    ("xl", p("prusa-xl", "Prusa XL", [360.0, 360.0])),
    (
        "k1 max",
        p("creality-k1-max", "Creality K1 Max", [300.0, 300.0]),
    ),
    ("k1c", p("creality-k1c", "Creality K1C", [220.0, 220.0])),
    ("k1", p("creality-k1", "Creality K1", [220.0, 220.0])),
    (
        "ender-3 v3",
        p("ender3-v3", "Creality Ender-3 V3", [220.0, 220.0]),
    ),
    (
        "ender-3 max",
        p("ender3-max", "Creality Ender-3 Max", [300.0, 300.0]),
    ),
    ("ender-3", p("ender3", "Creality Ender-3", [220.0, 220.0])),
    ("ender 3", p("ender3", "Creality Ender-3", [220.0, 220.0])),
    ("ender-5", p("ender5", "Creality Ender-5", [220.0, 220.0])),
];

const fn p(id: &'static str, display_name: &'static str, bed: [f32; 2]) -> PrinterProfile {
    PrinterProfile {
        id,
        display_name,
        bed,
        origin: [0.0, 0.0],
    }
}

/// Match a declared model string against the built-in table.
pub fn lookup(printer_model: &str) -> Option<PrinterProfile> {
    let needle = printer_model.to_ascii_lowercase();
    TABLE
        .iter()
        .find(|(key, _)| needle.contains(key))
        .map(|(_, profile)| *profile)
}

/// Parse a PrusaSlicer-family `bed_shape` value: comma-separated `x*y` corner points.
/// Returns `(size, origin)`.
pub fn from_bed_shape(value: &str) -> Option<([f32; 2], [f32; 2])> {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    let mut seen = 0usize;

    for point in value.split(',') {
        let (xs, ys) = point.trim().split_once('x')?;
        let x: f32 = xs.trim().parse().ok()?;
        let y: f32 = ys.trim().parse().ok()?;
        min[0] = min[0].min(x);
        min[1] = min[1].min(y);
        max[0] = max[0].max(x);
        max[1] = max[1].max(y);
        seen += 1;
    }

    if seen < 2 {
        return None;
    }
    Some(([max[0] - min[0], max[1] - min[1]], min))
}

/// Round Cura's print bounds up to the nearest common bed size that still contains them.
pub fn from_print_bounds(max_x: f32, max_y: f32) -> [f32; 2] {
    const COMMON: [f32; 6] = [120.0, 180.0, 220.0, 250.0, 256.0, 300.0];
    let pick = |v: f32| {
        COMMON
            .iter()
            .copied()
            .find(|c| *c >= v)
            .unwrap_or_else(|| v.ceil())
    };
    [pick(max_x), pick(max_y)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_models_win_over_prefixes() {
        assert_eq!(lookup("Bambu Lab X1 Carbon").unwrap().id, "bambu-x1c");
        assert_eq!(lookup("Bambu Lab A1 mini").unwrap().id, "bambu-a1-mini");
        assert_eq!(lookup("Creality K1 Max").unwrap().id, "creality-k1-max");
        assert_eq!(lookup("MK4IS").unwrap().id, "prusa-mk4");
        assert!(lookup("Some Unknown Machine").is_none());
    }

    #[test]
    fn parses_bed_shape() {
        let (size, origin) = from_bed_shape("0x0,256x0,256x256,0x256").unwrap();
        assert_eq!(size, [256.0, 256.0]);
        assert_eq!(origin, [0.0, 0.0]);

        let (size, origin) = from_bed_shape("-100x-100,100x-100,100x100,-100x100").unwrap();
        assert_eq!(size, [200.0, 200.0]);
        assert_eq!(origin, [-100.0, -100.0]);
    }

    #[test]
    fn rounds_print_bounds_to_a_plausible_bed() {
        assert_eq!(from_print_bounds(140.0, 90.0), [180.0, 120.0]);
        assert_eq!(from_print_bounds(251.0, 251.0), [256.0, 256.0]);
    }
}
