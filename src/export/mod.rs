mod dialog;
mod png;
mod svg;

use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::document::Document;
use crate::document::{DocumentRect, ObjectId, ObjectKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Png { scale: u32 },
}

impl ExportFormat {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Svg => "SVG",
            Self::Png { scale: 1 } => "PNG 1x",
            Self::Png { scale: 2 } => "PNG 2x",
            Self::Png { scale: 4 } => "PNG 4x",
            Self::Png { .. } => "PNG",
        }
    }
}

#[derive(Debug)]
pub enum ExportError {
    EmptyDocument,
    InvalidDimensions,
    ImageTooLarge { width: u32, height: u32 },
    FileExists(PathBuf),
    Svg(resvg::usvg::Error),
    Png(String),
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDocument => formatter.write_str("There is no artwork to export"),
            Self::InvalidDimensions => formatter.write_str("The artwork has invalid dimensions"),
            Self::ImageTooLarge { width, height } => {
                write!(formatter, "The PNG is too large ({width} x {height})")
            }
            Self::FileExists(path) => write!(
                formatter,
                "{} already exists; choose its full name to confirm replacement",
                path.display()
            ),
            Self::Svg(error) => write!(formatter, "SVG rendering failed: {error}"),
            Self::Png(error) => write!(formatter, "PNG encoding failed: {error}"),
            Self::Io(error) => write!(formatter, "The file could not be saved: {error}"),
        }
    }
}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn export_with_dialog(document: &Document, format: ExportFormat) -> Result<bool, ExportError> {
    let Some(target) = dialog::save_target(format) else {
        return Ok(false);
    };
    if target.path.exists() && !target.overwrite_confirmed {
        return Err(ExportError::FileExists(target.path));
    }
    let exported = svg::serialize(document)?;
    match target.format {
        ExportFormat::Svg => fs::write(target.path, exported.source.as_bytes())?,
        ExportFormat::Png { scale } => {
            png::write(&target.path, &exported, scale)?;
        }
    }
    Ok(true)
}

pub(crate) fn serialize_layer_content_for_canvas(
    document: &Document,
    layer_index: usize,
    viewport: DocumentRect,
    previews: &[(ObjectId, &ObjectKind)],
    extra: Option<&ObjectKind>,
    include_opacity: bool,
) -> Result<String, ExportError> {
    svg::serialize_layer(
        document,
        layer_index,
        viewport,
        previews,
        extra,
        include_opacity,
    )
}
