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
