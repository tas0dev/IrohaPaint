use std::fmt;

use super::*;

const MAGIC: &[u8; 8] = b"IROHAPNT";
const VERSION: u32 = 2;
const MAX_ITEMS: usize = 1_000_000;
const MAX_STRING: usize = 1024 * 1024;

#[derive(Debug)]
pub struct ProjectDecodeError(&'static str);

impl fmt::Display for ProjectDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for ProjectDecodeError {}

impl Document {
    pub fn to_project_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        writer.bytes(MAGIC);
        writer.u32(VERSION);
        match self.properties.canvas_size {
            CanvasSize::FitArtwork => writer.u8(0),
            CanvasSize::Custom { width, height } => {
                writer.u8(1);
                writer.f32(width);
                writer.f32(height);
            }
        }
        writer.color(self.properties.background);
        writer.len(self.folders.len());
        for folder in &self.folders {
            writer.u64(folder.id.0);
            writer.string(&folder.name);
            writer.bool(folder.visible);
            writer.bool(folder.expanded);
        }
        writer.len(self.layers.len());
        for layer in &self.layers {
            writer.string(&layer.name);
            writer.bool(layer.visible);
            writer.bool(layer.clipped);
            writer.bool(layer.locked);
            writer.bool(layer.alpha_locked);
            writer.f32(layer.opacity);
            writer.u64(layer.folder.map_or(0, |id| id.0));
            let tiles = layer.paint.tiles().collect::<Vec<_>>();
            writer.len(tiles.len());
            for tile in tiles {
                writer.i32(tile.x());
                writer.i32(tile.y());
                writer.len(tile.pixels().len());
                writer.bytes(tile.pixels());
            }
            writer.len(layer.objects.len());
            for object in &layer.objects {
                writer.u64(object.id.0);
                writer.object_kind(&object.kind);
            }
        }
        writer.u64(self.next_object_id);
        writer.u64(self.next_folder_id);
        writer.finish()
    }

    pub fn from_project_bytes(bytes: &[u8]) -> Result<Self, ProjectDecodeError> {
        let mut reader = Reader::new(bytes);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(ProjectDecodeError("This is not an IrohaPaint project"));
        }
        let version = reader.u32()?;
        if !matches!(version, 1 | VERSION) {
            return Err(ProjectDecodeError("This project version is not supported"));
        }
        let canvas_size = match reader.u8()? {
            0 => CanvasSize::FitArtwork,
            1 => CanvasSize::Custom {
                width: reader.finite_f32()?,
                height: reader.finite_f32()?,
            },
            _ => return Err(ProjectDecodeError("The canvas size is invalid")),
        };
        if let CanvasSize::Custom { width, height } = canvas_size
            && (width <= 0.0 || height <= 0.0)
        {
            return Err(ProjectDecodeError("The canvas dimensions are invalid"));
        }
        let background = reader.color()?;
        let folder_count = reader.len()?;
        let mut folders = Vec::with_capacity(folder_count);
        for _ in 0..folder_count {
            let id = FolderId(reader.nonzero_u64()?);
            folders.push(LayerFolder {
                id,
                name: reader.string()?,
                visible: reader.bool()?,
                expanded: reader.bool()?,
            });
        }
        let layer_count = reader.len()?;
        if layer_count == 0 {
            return Err(ProjectDecodeError("The project has no layers"));
        }
        let mut layers = Vec::with_capacity(layer_count);
        for layer_index in 0..layer_count {
            let name = reader.string()?;
            let visible = reader.bool()?;
            let mut clipped = reader.bool()?;
            let locked = version >= 2 && reader.bool()?;
            let alpha_locked = version >= 2 && reader.bool()?;
            let opacity = if version >= 2 {
                reader.finite_f32()?.clamp(0.0, 1.0)
            } else {
                1.0
            };
            if layer_index == 0 {
                clipped = false;
            }
            let folder_value = reader.u64()?;
            let folder = (folder_value != 0).then_some(FolderId(folder_value));
            if folder.is_some_and(|id| !folders.iter().any(|entry| entry.id == id)) {
                return Err(ProjectDecodeError("A layer refers to a missing folder"));
            }
            let mut paint = PaintLayer::default();
            for _ in 0..reader.len()? {
                let x = reader.i32()?;
                let y = reader.i32()?;
                let pixels = reader.byte_vec()?;
                if !paint.insert_tile(x, y, pixels) {
                    return Err(ProjectDecodeError("A paint tile has an invalid size"));
                }
            }
            let object_count = reader.len()?;
            let mut objects = Vec::with_capacity(object_count);
            for _ in 0..object_count {
                objects.push(DocumentObject {
                    id: ObjectId(reader.nonzero_u64()?),
                    kind: reader.object_kind()?,
                });
            }
            layers.push(Layer {
                name,
                objects,
                paint,
                visible,
                clipped,
                locked,
                alpha_locked,
                opacity,
                folder,
            });
        }
        let next_object_id = reader.nonzero_u64()?;
        let next_folder_id = reader.nonzero_u64()?;
        reader.finish()?;
        let maximum_object_id = layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
            .map(|object| object.id.0)
            .max()
            .unwrap_or(0);
        let maximum_folder_id = folders.iter().map(|folder| folder.id.0).max().unwrap_or(0);
        Ok(Self {
            selected_layer: Some(layers.len() - 1),
            selected_object: None,
            layers,
            folders,
            next_object_id: next_object_id.max(maximum_object_id.saturating_add(1)),
            next_folder_id: next_folder_id.max(maximum_folder_id.saturating_add(1)),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            properties: DocumentProperties {
                canvas_size,
                background,
            },
            modified: false,
        })
    }
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn finish(self) -> Vec<u8> {
        self.0
    }
    fn bytes(&mut self, value: &[u8]) {
        self.0.extend_from_slice(value);
    }
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }
    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
    fn f32(&mut self, value: f32) {
        self.bytes(&value.to_le_bytes());
    }
    fn len(&mut self, value: usize) {
        self.u32(value.min(u32::MAX as usize) as u32);
    }
    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }
    fn point(&mut self, value: DocumentPoint) {
        self.f32(value.x);
        self.f32(value.y);
    }
    fn rect(&mut self, value: DocumentRect) {
        self.f32(value.x);
        self.f32(value.y);
        self.f32(value.width);
        self.f32(value.height);
    }
    fn color(&mut self, value: DocumentColor) {
        self.bytes(&[value.red, value.green, value.blue, value.alpha]);
    }
    fn path(&mut self, path: &BezierPath) {
        self.bool(path.closed);
        self.len(path.nodes.len());
        for node in &path.nodes {
            self.point(node.position);
            self.point(node.handle_in);
            self.point(node.handle_out);
            self.u8(match node.kind {
                NodeKind::Corner => 0,
                NodeKind::Smooth => 1,
                NodeKind::Symmetric => 2,
            });
            self.f32(node.width);
        }
    }
    fn stroke(&mut self, value: StrokeStyle) {
        self.f32(value.width);
        self.f32(value.minimum_width);
        self.f32(value.taper_start);
        self.f32(value.taper_end);
        self.f32(value.tip_roundness);
        self.f32(value.tip_angle);
        self.u8(match value.cap {
            StrokeCap::Butt => 0,
            StrokeCap::Round => 1,
            StrokeCap::Square => 2,
        });
        self.u8(match value.join {
            StrokeJoin::Miter => 0,
            StrokeJoin::Round => 1,
            StrokeJoin::Bevel => 2,
        });
        self.color(value.color);
    }
    fn style(&mut self, value: ObjectStyle) {
        self.stroke(value.stroke);
        self.color(value.fill);
    }
    fn object_kind(&mut self, kind: &ObjectKind) {
        match kind {
            ObjectKind::Rectangle { bounds, style } => {
                self.u8(0);
                self.rect(*bounds);
                self.style(*style);
            }
            ObjectKind::Ellipse { bounds, style } => {
                self.u8(1);
                self.rect(*bounds);
                self.style(*style);
            }
            ObjectKind::Path {
                path,
                style,
                variable_width,
                cutouts,
            } => {
                self.u8(2);
                self.path(path);
                self.style(*style);
                self.bool(*variable_width);
                self.len(cutouts.len());
                for cutout in cutouts {
                    self.path(cutout);
                }
            }
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], ProjectDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProjectDecodeError("The project is too large"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProjectDecodeError("The project file is incomplete"))?;
        self.offset = end;
        Ok(value)
    }
    fn finish(&self) -> Result<(), ProjectDecodeError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProjectDecodeError(
                "The project contains unexpected trailing data",
            ))
        }
    }
    fn u8(&mut self) -> Result<u8, ProjectDecodeError> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, ProjectDecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ProjectDecodeError("A boolean value is invalid")),
        }
    }
    fn u32(&mut self) -> Result<u32, ProjectDecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, ProjectDecodeError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, ProjectDecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn nonzero_u64(&mut self) -> Result<u64, ProjectDecodeError> {
        let value = self.u64()?;
        if value == 0 {
            Err(ProjectDecodeError("An identifier is invalid"))
        } else {
            Ok(value)
        }
    }
    fn f32(&mut self) -> Result<f32, ProjectDecodeError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn finite_f32(&mut self) -> Result<f32, ProjectDecodeError> {
        let value = self.f32()?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(ProjectDecodeError("A number is invalid"))
        }
    }
    fn len(&mut self) -> Result<usize, ProjectDecodeError> {
        let value = self.u32()? as usize;
        if value <= MAX_ITEMS {
            Ok(value)
        } else {
            Err(ProjectDecodeError("The project contains too many items"))
        }
    }
    fn byte_vec(&mut self) -> Result<Vec<u8>, ProjectDecodeError> {
        let length = self.len()?;
        Ok(self.take(length)?.to_vec())
    }
    fn string(&mut self) -> Result<String, ProjectDecodeError> {
        let length = self.u32()? as usize;
        if length > MAX_STRING {
            return Err(ProjectDecodeError("A name is too long"));
        }
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| ProjectDecodeError("A name is not valid UTF-8"))
    }
    fn point(&mut self) -> Result<DocumentPoint, ProjectDecodeError> {
        Ok(DocumentPoint::new(self.finite_f32()?, self.finite_f32()?))
    }
    fn rect(&mut self) -> Result<DocumentRect, ProjectDecodeError> {
        Ok(DocumentRect {
            x: self.finite_f32()?,
            y: self.finite_f32()?,
            width: self.finite_f32()?,
            height: self.finite_f32()?,
        })
    }
    fn color(&mut self) -> Result<DocumentColor, ProjectDecodeError> {
        let value = self.take(4)?;
        Ok(DocumentColor::rgba(value[0], value[1], value[2], value[3]))
    }
    fn path(&mut self) -> Result<BezierPath, ProjectDecodeError> {
        let closed = self.bool()?;
        let count = self.len()?;
        if count == 0 {
            return Err(ProjectDecodeError("A path has no nodes"));
        }
        let mut nodes = Vec::with_capacity(count);
        for _ in 0..count {
            let position = self.point()?;
            let handle_in = self.point()?;
            let handle_out = self.point()?;
            let kind = match self.u8()? {
                0 => NodeKind::Corner,
                1 => NodeKind::Smooth,
                2 => NodeKind::Symmetric,
                _ => return Err(ProjectDecodeError("A node type is invalid")),
            };
            nodes.push(BezierNode {
                position,
                handle_in,
                handle_out,
                kind,
                width: self.finite_f32()?,
            });
        }
        Ok(BezierPath { nodes, closed })
    }
    fn stroke(&mut self) -> Result<StrokeStyle, ProjectDecodeError> {
        let width = self.finite_f32()?;
        let minimum_width = self.finite_f32()?;
        let taper_start = self.finite_f32()?;
        let taper_end = self.finite_f32()?;
        let tip_roundness = self.finite_f32()?;
        let tip_angle = self.finite_f32()?;
        let cap = match self.u8()? {
            0 => StrokeCap::Butt,
            1 => StrokeCap::Round,
            2 => StrokeCap::Square,
            _ => return Err(ProjectDecodeError("A stroke cap is invalid")),
        };
        let join = match self.u8()? {
            0 => StrokeJoin::Miter,
            1 => StrokeJoin::Round,
            2 => StrokeJoin::Bevel,
            _ => return Err(ProjectDecodeError("A stroke join is invalid")),
        };
        Ok(StrokeStyle {
            width,
            minimum_width,
            taper_start,
            taper_end,
            tip_roundness,
            tip_angle,
            cap,
            join,
            color: self.color()?,
        })
    }
    fn style(&mut self) -> Result<ObjectStyle, ProjectDecodeError> {
        Ok(ObjectStyle {
            stroke: self.stroke()?,
            fill: self.color()?,
        })
    }
    fn object_kind(&mut self) -> Result<ObjectKind, ProjectDecodeError> {
        match self.u8()? {
            0 => Ok(ObjectKind::Rectangle {
                bounds: self.rect()?,
                style: self.style()?,
            }),
            1 => Ok(ObjectKind::Ellipse {
                bounds: self.rect()?,
                style: self.style()?,
            }),
            2 => {
                let path = self.path()?;
                let style = self.style()?;
                let variable_width = self.bool()?;
                let count = self.len()?;
                let mut cutouts = Vec::with_capacity(count);
                for _ in 0..count {
                    cutouts.push(self.path()?);
                }
                Ok(ObjectKind::Path {
                    path,
                    style,
                    variable_width,
                    cutouts,
                })
            }
            _ => Err(ProjectDecodeError("An object type is invalid")),
        }
    }
}
