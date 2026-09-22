use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("unsupported container: {0}")]
    UnsupportedContainer(String),

    #[error("no plate G-code found inside the archive")]
    NoGcodeInArchive,

    #[error("file contains no printable moves")]
    NoMoves,

    /// A 3D model — an `.stl`, a `.step` — rather than the G-code a slicer makes from one.
    #[error(".{0} is a 3D model, not sliced G-code")]
    ModelFile(String),

    /// The file stops before the end of the print its own header describes: cut off while it was
    /// being copied or downloaded.
    #[error(
        "file ends at line {line}, before the end of the print ({layers_read} of {layers_declared} layers)"
    )]
    Truncated {
        line: u64,
        layers_read: u32,
        layers_declared: u32,
    },
}

impl Error {
    /// A stable identifier for this failure.
    ///
    /// `Display` is this crate's own English, which is what the CLI prints and what a Rust
    /// caller sees. The desktop app is not in English, and it cannot translate prose without
    /// matching on it -- a match that breaks silently the day a sentence here is reworded. So
    /// the code travels alongside the message and the interface translates the code.
    ///
    /// These strings are part of the IPC contract: rename one and the app falls back to the
    /// English message.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Zip(_) => "archive",
            Error::UnsupportedContainer(_) => "unsupported_container",
            Error::NoGcodeInArchive => "no_gcode_in_archive",
            Error::NoMoves => "no_moves",
            Error::ModelFile(_) => "model_file",
            Error::Truncated { .. } => "truncated",
        }
    }

    /// The machine-readable particulars of a failure, for an interface that wants to say more
    /// than the code — the line a file was cut at, the kind of file that was dropped. `Null` when
    /// there is nothing beyond the code. Part of the IPC contract, like the code.
    pub fn detail(&self) -> serde_json::Value {
        match self {
            Error::ModelFile(extension) => serde_json::json!({ "extension": extension }),
            Error::Truncated {
                line,
                layers_read,
                layers_declared,
            } => serde_json::json!({
                "line": line,
                "layersRead": layers_read,
                "layersDeclared": layers_declared,
            }),
            _ => serde_json::Value::Null,
        }
    }
}

/// Non-fatal degradations. Layer 2 collects these and the UI surfaces them; a job never stops
/// because of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    UnknownDialect,
    NoLayerMarkers,
    NoFeatureMarkers,
    DerivedWidth,
    NoPrinterProfile,
    UnsupportedArcWithoutOffsets,
}

impl Warning {
    /// A stable identifier, for the same reason [`Error::code`] has one. This is what reaches
    /// the front end inside `meta.warnings`.
    pub fn code(&self) -> &'static str {
        match self {
            Warning::UnknownDialect => "unknown_dialect",
            Warning::NoLayerMarkers => "no_layer_markers",
            Warning::NoFeatureMarkers => "no_feature_markers",
            Warning::DerivedWidth => "derived_width",
            Warning::NoPrinterProfile => "no_printer_profile",
            Warning::UnsupportedArcWithoutOffsets => "arc_without_offsets",
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Warning::UnknownDialect => {
                "Slicer could not be identified. Geometry is exact; layer and feature colouring \
                 are approximated."
            }
            Warning::NoLayerMarkers => {
                "No layer-change comments found. Layers were inferred from Z movement."
            }
            Warning::NoFeatureMarkers => {
                "No feature-type comments found. Walls, infill and support are drawn identically."
            }
            Warning::DerivedWidth => {
                "Extrusion width is not declared by this slicer. It was computed from extruder \
                 movement and layer height."
            }
            Warning::NoPrinterProfile => {
                "Printer model was not recognised. A generic build plate is shown."
            }
            Warning::UnsupportedArcWithoutOffsets => {
                "An arc move had no I/J offsets and was drawn as a straight line."
            }
        };
        f.write_str(s)
    }
}
