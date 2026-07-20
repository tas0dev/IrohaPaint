use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::document::{DocumentColor, StrokeCap, StrokeJoin, StrokeStyle};

const BRUSH_DIRECTORY: &str = "resources/brushes";
const BRUSH_EXTENSION: &str = "irohabrush";

#[derive(Debug)]
pub enum BrushFileError {
    Io(std::io::Error),
    Invalid { path: PathBuf, message: String },
    NoBrushes(PathBuf),
}

impl fmt::Display for BrushFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Invalid { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::NoBrushes(path) => {
                write!(formatter, "No brush files found in {}", path.display())
            }
        }
    }
}

impl std::error::Error for BrushFileError {}

impl From<std::io::Error> for BrushFileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrushTip {
    Round,
    Ellipse { roundness: f32, angle: f32 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrushKind {
    #[default]
    Line,
    Paint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushDefinition {
    pub name: String,
    pub kind: BrushKind,
    pub tip: BrushTip,
    pub width: f32,
    pub paint_width: f32,
    pub minimum_width: f32,
    pub smoothing: f32,
    pub streamline: f32,
    pub taper_start: f32,
    pub taper_end: f32,
    pub color: DocumentColor,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
}

impl BrushDefinition {
    pub fn stroke_style(&self) -> StrokeStyle {
        let (tip_roundness, tip_angle) = match self.tip {
            BrushTip::Round => (1.0, 0.0),
            BrushTip::Ellipse { roundness, angle } => {
                (roundness.clamp(0.05, 1.0), angle.to_radians())
            }
        };
        StrokeStyle {
            width: self.width.max(0.1),
            minimum_width: self.minimum_width.clamp(0.01, 1.0),
            taper_start: self.taper_start.clamp(0.0, 1.0),
            taper_end: self.taper_end.clamp(0.0, 1.0),
            tip_roundness,
            tip_angle,
            cap: self.cap,
            join: self.join,
            color: self.color,
        }
    }

    pub fn fitting_tolerance(&self, zoom: f32) -> f32 {
        let smoothing = self.smoothing.clamp(0.0, 1.0);
        let streamline = self.streamline.clamp(0.0, 1.0);
        (0.45 + smoothing * 1.35 + streamline * 0.9) / zoom.max(0.01)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushLibrary {
    presets: Vec<BrushDefinition>,
    active_line: usize,
    active_paint: usize,
}

impl BrushLibrary {
    pub fn presets(&self) -> &[BrushDefinition] {
        &self.presets
    }

    pub fn active_index(&self, kind: BrushKind) -> usize {
        match kind {
            BrushKind::Line => self.active_line,
            BrushKind::Paint => self.active_paint,
        }
    }

    pub fn active(&self, kind: BrushKind) -> &BrushDefinition {
        &self.presets[self.active_index(kind)]
    }

    pub fn select(&mut self, kind: BrushKind, index: usize) {
        if self
            .presets
            .get(index)
            .is_some_and(|brush| brush.kind == kind)
        {
            match kind {
                BrushKind::Line => self.active_line = index,
                BrushKind::Paint => self.active_paint = index,
            }
        }
    }

    pub fn update_active(&mut self, kind: BrushKind, update: impl FnOnce(&mut BrushDefinition)) {
        let index = self.active_index(kind);
        update(&mut self.presets[index]);
        sanitize(&mut self.presets[index]);
    }

    pub fn save_active_as_file(
        &mut self,
        kind: BrushKind,
        name: &str,
    ) -> Result<PathBuf, BrushFileError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BrushFileError::Invalid {
                path: brush_directory(),
                message: String::from("Brush name is empty"),
            });
        }
        let mut brush = self.active(kind).clone();
        brush.name = name.to_owned();
        sanitize(&mut brush);

        let directory = brush_directory();
        fs::create_dir_all(&directory)?;
        let path = unique_brush_path(&directory, name);
        fs::write(&path, serialize_brush(&brush))?;
        self.presets.push(brush);
        self.select(kind, self.presets.len() - 1);
        Ok(path)
    }

    pub fn reload_from_disk(&mut self) -> Result<(), BrushFileError> {
        let loaded = load_brushes(&brush_directory())?;
        self.presets = loaded;
        self.active_line = preferred_brush_index(&self.presets, BrushKind::Line, "Clean Inking");
        self.active_paint = preferred_brush_index(&self.presets, BrushKind::Paint, "Flat Color");
        Ok(())
    }
}

impl Default for BrushLibrary {
    fn default() -> Self {
        if let Ok(presets) = load_brushes(&brush_directory()) {
            let active_line = preferred_brush_index(&presets, BrushKind::Line, "Clean Inking");
            let active_paint = preferred_brush_index(&presets, BrushKind::Paint, "Flat Color");
            return Self {
                presets,
                active_line,
                active_paint,
            };
        }
        Self {
            presets: vec![
                BrushDefinition {
                    tip: BrushTip::Ellipse {
                        roundness: 0.82,
                        angle: -45.0,
                    },
                    ..preset(BrushKind::Line, "Clean Inking", 2.5, 18.0, 0.72, 0.35)
                },
                preset(BrushKind::Line, "Smooth Pencil", 1.8, 30.0, 0.9, 0.15),
                preset(BrushKind::Line, "Monoline", 3.0, 48.0, 0.45, 0.0),
                preset(BrushKind::Paint, "Flat Color", 3.0, 48.0, 0.65, 0.0),
            ],
            active_line: 0,
            active_paint: 3,
        }
    }
}

fn preferred_brush_index(brushes: &[BrushDefinition], kind: BrushKind, name: &str) -> usize {
    brushes
        .iter()
        .position(|brush| brush.kind == kind && brush.name == name)
        .or_else(|| brushes.iter().position(|brush| brush.kind == kind))
        .unwrap_or(0)
}

fn brush_directory() -> PathBuf {
    PathBuf::from(BRUSH_DIRECTORY)
}

fn load_brushes(directory: &Path) -> Result<Vec<BrushDefinition>, BrushFileError> {
    let mut paths = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case(BRUSH_EXTENSION))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut brushes = Vec::with_capacity(paths.len());
    for path in paths {
        let source = fs::read_to_string(&path)?;
        brushes.push(parse_brush(&path, &source)?);
    }
    if brushes.is_empty() {
        return Err(BrushFileError::NoBrushes(directory.to_owned()));
    }
    Ok(brushes)
}

fn parse_brush(path: &Path, source: &str) -> Result<BrushDefinition, BrushFileError> {
    let value = |key: &str| {
        source.lines().find_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (candidate, value) = line.split_once('=')?;
            (candidate.trim() == key).then(|| value.trim())
        })
    };
    let required = |key: &str| {
        value(key).ok_or_else(|| BrushFileError::Invalid {
            path: path.to_owned(),
            message: format!("Missing {key}"),
        })
    };
    let number = |key: &str| -> Result<f32, BrushFileError> {
        required(key)?
            .parse::<f32>()
            .map_err(|_| BrushFileError::Invalid {
                path: path.to_owned(),
                message: format!("Invalid {key}"),
            })
    };
    let optional_number = |key: &str, fallback: f32| -> Result<f32, BrushFileError> {
        let Some(raw) = value(key) else {
            return Ok(fallback);
        };
        raw.parse::<f32>().map_err(|_| BrushFileError::Invalid {
            path: path.to_owned(),
            message: format!("Invalid {key}"),
        })
    };
    if required("version")? != "2" {
        return Err(BrushFileError::Invalid {
            path: path.to_owned(),
            message: String::from("Unsupported version"),
        });
    }
    let tip = match required("tip")? {
        "round" => BrushTip::Round,
        "ellipse" => BrushTip::Ellipse {
            roundness: number("tip_roundness")?,
            angle: number("tip_angle")?,
        },
        _ => return invalid_value(path, "tip"),
    };
    let cap = match required("cap")? {
        "butt" => StrokeCap::Butt,
        "round" => StrokeCap::Round,
        "square" => StrokeCap::Square,
        _ => return invalid_value(path, "cap"),
    };
    let join = match required("join")? {
        "miter" => StrokeJoin::Miter,
        "round" => StrokeJoin::Round,
        "bevel" => StrokeJoin::Bevel,
        _ => return invalid_value(path, "join"),
    };
    let mut brush = BrushDefinition {
        name: required("name")?.to_owned(),
        kind: match value("kind").unwrap_or("line") {
            "line" => BrushKind::Line,
            "paint" => BrushKind::Paint,
            _ => return invalid_value(path, "kind"),
        },
        tip,
        width: number("width")?,
        paint_width: optional_number("paint_width", 40.0)?,
        minimum_width: number("minimum_width")?,
        smoothing: number("smoothing")?,
        streamline: number("streamline")?,
        taper_start: number("taper_start")?,
        taper_end: number("taper_end")?,
        color: DocumentColor::from_hex(required("color")?).ok_or_else(|| {
            BrushFileError::Invalid {
                path: path.to_owned(),
                message: String::from("Invalid color"),
            }
        })?,
        cap,
        join,
    };
    sanitize(&mut brush);
    Ok(brush)
}

fn invalid_value<T>(path: &Path, key: &str) -> Result<T, BrushFileError> {
    Err(BrushFileError::Invalid {
        path: path.to_owned(),
        message: format!("Invalid {key}"),
    })
}

fn serialize_brush(brush: &BrushDefinition) -> String {
    let (tip, roundness, angle) = match brush.tip {
        BrushTip::Round => ("round", 1.0, 0.0),
        BrushTip::Ellipse { roundness, angle } => ("ellipse", roundness, angle),
    };
    format!(
        "version=2\nname={}\nkind={}\ntip={}\ntip_roundness={}\ntip_angle={}\nwidth={}\npaint_width={}\nminimum_width={}\nsmoothing={}\nstreamline={}\ntaper_start={}\ntaper_end={}\ncolor={}\ncap={}\njoin={}\n",
        brush.name.replace(['\r', '\n'], " "),
        match brush.kind {
            BrushKind::Line => "line",
            BrushKind::Paint => "paint",
        },
        tip,
        roundness,
        angle,
        brush.width,
        brush.paint_width,
        brush.minimum_width,
        brush.smoothing,
        brush.streamline,
        brush.taper_start,
        brush.taper_end,
        brush.color.to_hex(),
        cap_name(brush.cap),
        join_name(brush.join),
    )
}

fn unique_brush_path(directory: &Path, name: &str) -> PathBuf {
    let stem = sanitize_file_name(name);
    let mut path = directory.join(format!("{stem}.{BRUSH_EXTENSION}"));
    let mut suffix = 2;
    while path.exists() {
        path = directory.join(format!("{stem}-{suffix}.{BRUSH_EXTENSION}"));
        suffix += 1;
    }
    path
}

fn sanitize_file_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        String::from("brush")
    } else {
        sanitized.to_owned()
    }
}

fn cap_name(cap: StrokeCap) -> &'static str {
    match cap {
        StrokeCap::Butt => "butt",
        StrokeCap::Round => "round",
        StrokeCap::Square => "square",
    }
}

fn join_name(join: StrokeJoin) -> &'static str {
    match join {
        StrokeJoin::Miter => "miter",
        StrokeJoin::Round => "round",
        StrokeJoin::Bevel => "bevel",
    }
}

fn preset(
    kind: BrushKind,
    name: &str,
    width: f32,
    paint_width: f32,
    smoothing: f32,
    taper: f32,
) -> BrushDefinition {
    BrushDefinition {
        name: name.to_owned(),
        kind,
        tip: BrushTip::Round,
        width,
        paint_width,
        minimum_width: 0.2,
        smoothing,
        streamline: 0.5,
        taper_start: taper,
        taper_end: taper,
        color: DocumentColor::BLACK,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
    }
}

fn sanitize(brush: &mut BrushDefinition) {
    brush.width = brush.width.clamp(0.1, 256.0);
    brush.paint_width = brush.paint_width.clamp(1.0, 400.0);
    brush.minimum_width = brush.minimum_width.clamp(0.0, 1.0);
    brush.smoothing = brush.smoothing.clamp(0.0, 1.0);
    brush.streamline = brush.streamline.clamp(0.0, 1.0);
    brush.taper_start = brush.taper_start.clamp(0.0, 1.0);
    brush.taper_end = brush.taper_end.clamp(0.0, 1.0);
    if let BrushTip::Ellipse { roundness, angle } = &mut brush.tip {
        *roundness = roundness.clamp(0.05, 1.0);
        *angle = angle.rem_euclid(360.0);
    }
}
