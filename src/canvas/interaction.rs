use viewkit::prelude::Point;

use crate::document::{DocumentPoint, DocumentRect, ObjectId, ObjectKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeDraftKind {
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl ResizeHandle {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomRight,
        Self::BottomLeft,
    ];

    pub fn position(self, bounds: DocumentRect) -> DocumentPoint {
        match self {
            Self::TopLeft => DocumentPoint::new(bounds.x, bounds.y),
            Self::TopRight => DocumentPoint::new(bounds.x + bounds.width, bounds.y),
            Self::BottomRight => {
                DocumentPoint::new(bounds.x + bounds.width, bounds.y + bounds.height)
            }
            Self::BottomLeft => DocumentPoint::new(bounds.x, bounds.y + bounds.height),
        }
    }

    pub fn opposite(self, bounds: DocumentRect) -> DocumentPoint {
        match self {
            Self::TopLeft => DocumentPoint::new(bounds.x + bounds.width, bounds.y + bounds.height),
            Self::TopRight => DocumentPoint::new(bounds.x, bounds.y + bounds.height),
            Self::BottomRight => DocumentPoint::new(bounds.x, bounds.y),
            Self::BottomLeft => DocumentPoint::new(bounds.x + bounds.width, bounds.y),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Interaction {
    #[default]
    Idle,
    DrawingShape {
        kind: ShapeDraftKind,
        start: DocumentPoint,
        current: DocumentPoint,
    },
    DrawingPath {
        points: Vec<DocumentPoint>,
    },
    Panning {
        start_canvas: Point,
        start_pan: Point,
    },
    Moving {
        id: ObjectId,
        original: ObjectKind,
        start: DocumentPoint,
        current: DocumentPoint,
    },
    Resizing {
        id: ObjectId,
        original: ObjectKind,
        anchor: DocumentPoint,
        current: DocumentPoint,
        handle: ResizeHandle,
    },
}

impl Interaction {
    pub fn preview_kind(&self, object_id: ObjectId) -> Option<ObjectKind> {
        match self {
            Self::Moving {
                id,
                original,
                start,
                current,
            } if *id == object_id => {
                let delta = DocumentPoint::new(current.x - start.x, current.y - start.y);
                Some(translated_kind(original, delta))
            }
            Self::Resizing {
                id,
                original,
                anchor,
                current,
                ..
            } if *id == object_id => Some(resized_kind(
                original,
                DocumentRect::from_points(*anchor, *current),
            )),
            _ => None,
        }
    }
}

fn translated_kind(kind: &ObjectKind, delta: DocumentPoint) -> ObjectKind {
    match kind {
        ObjectKind::Rectangle { bounds } => ObjectKind::Rectangle {
            bounds: bounds.translated(delta),
        },
        ObjectKind::Ellipse { bounds } => ObjectKind::Ellipse {
            bounds: bounds.translated(delta),
        },
        ObjectKind::Path { points } => ObjectKind::Path {
            points: points
                .iter()
                .map(|point| DocumentPoint::new(point.x + delta.x, point.y + delta.y))
                .collect(),
        },
    }
}

fn resized_kind(kind: &ObjectKind, new_bounds: DocumentRect) -> ObjectKind {
    let old_bounds = kind_bounds(kind);
    match kind {
        ObjectKind::Rectangle { .. } => ObjectKind::Rectangle { bounds: new_bounds },
        ObjectKind::Ellipse { .. } => ObjectKind::Ellipse { bounds: new_bounds },
        ObjectKind::Path { points } => ObjectKind::Path {
            points: points
                .iter()
                .map(|point| {
                    DocumentPoint::new(
                        scale_axis(
                            point.x,
                            old_bounds.x,
                            old_bounds.width,
                            new_bounds.x,
                            new_bounds.width,
                        ),
                        scale_axis(
                            point.y,
                            old_bounds.y,
                            old_bounds.height,
                            new_bounds.y,
                            new_bounds.height,
                        ),
                    )
                })
                .collect(),
        },
    }
}

pub fn kind_bounds(kind: &ObjectKind) -> DocumentRect {
    match kind {
        ObjectKind::Rectangle { bounds } | ObjectKind::Ellipse { bounds } => *bounds,
        ObjectKind::Path { points } => {
            let Some(first) = points.first() else {
                return DocumentRect::default();
            };
            let (mut min_x, mut max_x) = (first.x, first.x);
            let (mut min_y, mut max_y) = (first.y, first.y);
            for point in &points[1..] {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
            DocumentRect {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            }
        }
    }
}

fn scale_axis(value: f32, old_start: f32, old_size: f32, new_start: f32, new_size: f32) -> f32 {
    if old_size.abs() <= f32::EPSILON {
        new_start
    } else {
        new_start + (value - old_start) / old_size * new_size
    }
}
