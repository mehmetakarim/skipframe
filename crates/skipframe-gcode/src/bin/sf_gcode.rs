//! `sf-gcode` -- the parser without the app around it.
//!
//! Used for phase-0 measurement and for reproducing parse bugs from a shell.
//!
//! ```text
//! sf-gcode parse <file> [--plate N] [--repeat N] [--out ir.bin]
//! sf-gcode plates <file.gcode.3mf>
//! sf-gcode synth <out.gcode> [--layers N] [--per-layer N] [--dialect prusa|orca|bambu|cura]
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("parse") => cmd_parse(&args[1..]),
        Some("plates") => cmd_plates(&args[1..]),
        Some("synth") => cmd_synth(&args[1..]),
        _ => {
            eprintln!("{}", USAGE);
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "sf-gcode parse <file> [--plate N] [--repeat N] [--out ir.bin]\n\
                     sf-gcode plates <file.gcode.3mf>\n\
                     sf-gcode synth <out.gcode> [--layers N] [--per-layer N] [--dialect NAME]";

type Res = Result<(), Box<dyn std::error::Error>>;

fn cmd_parse(args: &[String]) -> Res {
    let path = PathBuf::from(args.first().ok_or("missing file")?);
    let plate = flag(args, "--plate")
        .map(|v| v.parse::<u32>())
        .transpose()?;
    let repeat = flag(args, "--repeat")
        .map(|v| v.parse::<u32>())
        .transpose()?
        .unwrap_or(1);
    let out = flag(args, "--out").map(PathBuf::from);

    let size = std::fs::metadata(&path)?.len();

    let t = Instant::now();
    let key = skipframe_gcode::cache_key(&path)?;
    let hash_ms = t.elapsed().as_secs_f64() * 1000.0;

    let mut best = f64::MAX;
    let mut worst: f64 = 0.0;
    let mut total = 0.0;
    let mut ir = None;
    for _ in 0..repeat {
        let t = Instant::now();
        let parsed = skipframe_gcode::parse_file(&path, plate)?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        best = best.min(ms);
        worst = worst.max(ms);
        total += ms;
        ir = Some(parsed);
    }
    let ir = ir.expect("at least one run");

    let t = Instant::now();
    let buf = ir.encode();
    let encode_ms = t.elapsed().as_secs_f64() * 1000.0;

    let m = &ir.meta;
    println!("file            {}", path.display());
    println!("size            {:.1} MB", size as f64 / 1e6);
    println!("cache key       {key}");
    println!("hash            {hash_ms:.1} ms");
    println!(
        "parse           best {best:.1} ms | mean {:.1} ms | worst {worst:.1} ms  ({repeat} run(s))",
        total / repeat as f64
    );
    println!("encode          {encode_ms:.1} ms");
    println!("ir buffer       {:.1} MB", buf.len() as f64 / 1e6);
    println!(
        "throughput      {:.1} MB/s",
        (size as f64 / 1e6) / (best / 1000.0)
    );
    println!("--");
    println!(
        "dialect         {} {}",
        m.dialect,
        m.slicer_version.as_deref().unwrap_or("")
    );
    println!(
        "printer         {:?} -> {:?}",
        m.printer_model, m.printer_profile
    );
    println!("bed             {:?} origin {:?}", m.bed_size, m.bed_origin);
    println!("layers          {}", m.layer_count);
    println!("segments        {}", m.segment_count);
    println!(
        "extruding       {}",
        ir.feature_type
            .iter()
            .filter(|f| **f != skipframe_gcode::FeatureType::Travel.as_u8())
            .count()
    );
    println!("tools           {}", m.tool_count);
    println!("feature types   {}", m.has_feature_types);
    println!("declared width  {}", m.has_declared_width);
    println!("est. time       {:?} s", m.estimated_time_s);
    println!(
        "filament        {:?} g / {:?} mm",
        m.filament_grams, m.filament_mm
    );
    println!(
        "bounds          x {:.1}..{:.1}  y {:.1}..{:.1}  z {:.1}..{:.1}",
        m.bounds[0], m.bounds[3], m.bounds[1], m.bounds[4], m.bounds[2], m.bounds[5]
    );
    for w in &m.warnings {
        println!("warning         {w}");
    }

    if let Some(out) = out {
        std::fs::write(&out, &buf)?;
        println!("--\nwrote           {}", out.display());
    }
    Ok(())
}

fn cmd_plates(args: &[String]) -> Res {
    let path = PathBuf::from(args.first().ok_or("missing file")?);
    for p in skipframe_gcode::container::list_plates(&path)? {
        println!("{p}");
    }
    Ok(())
}

/// Generate a G-code file with a controllable path count, for benchmarking without shipping a
/// multi-hundred-megabyte fixture in the repository.
fn cmd_synth(args: &[String]) -> Res {
    let out = PathBuf::from(args.first().ok_or("missing output path")?);
    let layers: u32 = flag(args, "--layers")
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(570);
    let per_layer: u32 = flag(args, "--per-layer")
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(1316);
    let dialect = flag(args, "--dialect").unwrap_or_else(|| "prusa".into());

    let file = std::fs::File::create(&out)?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, file);
    write_synth(&mut w, &dialect, layers, per_layer)?;
    w.flush()?;

    let size = std::fs::metadata(&out)?.len();
    println!(
        "wrote {} -- {} layers x {} paths = {} paths, {:.1} MB",
        out.display(),
        layers,
        per_layer,
        layers as u64 * per_layer as u64,
        size as f64 / 1e6
    );
    Ok(())
}

fn write_synth(
    w: &mut impl Write,
    dialect: &str,
    layers: u32,
    per_layer: u32,
) -> std::io::Result<()> {
    let (banner, layer_marker, type_marker, emits_width) = match dialect {
        "cura" => (
            ";Generated with Cura_SteamEngine 5.7.0",
            "LAYER",
            "TYPE",
            false,
        ),
        "bambu" => (
            "; generated by BambuStudio 1.9.0.50",
            "CHANGE_LAYER",
            "FEATURE",
            true,
        ),
        "orca" => (
            "; generated by OrcaSlicer 2.1.1",
            "LAYER_CHANGE",
            "TYPE",
            true,
        ),
        _ => (
            "; generated by PrusaSlicer 2.8.1+win64",
            "LAYER_CHANGE",
            "TYPE",
            true,
        ),
    };
    let features: [&str; 4] = if type_marker == "FEATURE" {
        ["Outer wall", "Inner wall", "Sparse infill", "Support"]
    } else if dialect == "cura" {
        ["WALL-OUTER", "WALL-INNER", "FILL", "SUPPORT"]
    } else {
        [
            "External perimeter",
            "Perimeter",
            "Internal infill",
            "Support material",
        ]
    };

    writeln!(w, "{banner}")?;
    writeln!(w, "; layer_height = 0.2")?;
    writeln!(w, "; nozzle_diameter = 0.4")?;
    writeln!(w, "; filament_diameter = 1.75")?;
    writeln!(w, "; printer_model = MK4IS")?;
    writeln!(w, "; bed_shape = 0x0,250x0,250x210,0x210")?;
    writeln!(w, "G21")?;
    writeln!(w, "G90")?;
    writeln!(w, "M83")?;
    writeln!(w, "G28")?;

    for layer in 0..layers {
        let z = 0.2 + layer as f32 * 0.2;
        if layer_marker == "LAYER" {
            writeln!(w, ";LAYER:{layer}")?;
        } else {
            writeln!(w, ";{layer_marker}")?;
            writeln!(w, ";Z:{z:.3}")?;
        }
        // A travel to the start of the layer, then a spiral of extrusions.
        writeln!(w, "G1 X100.000 Y100.000 Z{z:.3} F9000")?;
        let mut feature_idx = usize::MAX;
        for i in 0..per_layer {
            let f = (i as usize * 4 / per_layer.max(1) as usize).min(3);
            if f != feature_idx {
                feature_idx = f;
                writeln!(w, ";{type_marker}:{}", features[f])?;
                if emits_width {
                    writeln!(w, ";WIDTH:0.45")?;
                }
            }
            let a = i as f32 * 0.05 + layer as f32 * 0.01;
            let r = 20.0 + (i as f32 * 0.01) % 40.0;
            let x = 100.0 + r * a.cos();
            let y = 100.0 + r * a.sin();
            writeln!(w, "G1 X{x:.3} Y{y:.3} E0.0421 F1800")?;
        }
        writeln!(w, "G1 E-0.8 F2400")?;
    }
    writeln!(w, "; filament used [g] = 42.7")?;
    writeln!(w, "; estimated printing time (normal mode) = 5h 12m 30s")?;
    Ok(())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}
