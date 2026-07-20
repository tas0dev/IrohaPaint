use viewkit::prelude::Point;

use crate::document::{
    BezierNode, BezierPath, DocumentPoint, DocumentRect, NodeComponent, ObjectId, ObjectKind,
};

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
    PlacingPathNode {
        path_id: Option<ObjectId>,
        original: Option<ObjectKind>,
        position: DocumentPoint,
        handle_out: DocumentPoint,
    },
    ClosingPath {
        id: ObjectId,
    },
    EditingPathNode {
        id: ObjectId,
        original: ObjectKind,
        node_index: usize,
        component: NodeComponent,
        current: DocumentPoint,
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
            Self::EditingPathNode {
                id,
                original,
                node_index,
                component,
                current,
            } if *id == object_id => Some(edited_path_kind(
                original,
                *node_index,
                *component,
                *current,
            )),
            Self::PlacingPathNode {
                path_id: Some(id),
                original: Some(original),
                position,
                handle_out,
            } if *id == object_id => Some(appended_path_kind(original, *position, *handle_out)),
            _ => None,
        }
    }
}

fn appended_path_kind(
    original: &ObjectKind,
    position: DocumentPoint,
    handle_out: DocumentPoint,
) -> ObjectKind {
    let ObjectKind::Path { path } = original else {
        return original.clone();
    };
    let mut path = path.clone();
    path.push_node(BezierNode::smooth(position, handle_out));
    ObjectKind::Path { path }
}

fn translated_kind(kind: &ObjectKind, delta: DocumentPoint) -> ObjectKind {
    match kind {
        ObjectKind::Rectangle { bounds } => ObjectKind::Rectangle {
            bounds: bounds.translated(delta),
        },
        ObjectKind::Ellipse { bounds } => ObjectKind::Ellipse {
            bounds: bounds.translated(delta),
        },
        ObjectKind::Path { path } => ObjectKind::Path {
            path: transformed_path(path, |point| {
                DocumentPoint::new(point.x + delta.x, point.y + delta.y)
            }),
        },
    }
}

fn resized_kind(kind: &ObjectKind, new_bounds: DocumentRect) -> ObjectKind {
    let old_bounds = kind_bounds(kind);
    match kind {
        ObjectKind::Rectangle { .. } => ObjectKind::Rectangle { bounds: new_bounds },
        ObjectKind::Ellipse { .. } => ObjectKind::Ellipse { bounds: new_bounds },
        ObjectKind::Path { path } => ObjectKind::Path {
            path: transformed_path(path, |point| {
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
            }),
        },
    }
}

pub fn kind_bounds(kind: &ObjectKind) -> DocumentRect {
    match kind {
        ObjectKind::Rectangle { bounds } | ObjectKind::Ellipse { bounds } => *bounds,
        ObjectKind::Path { path } => {
            let Some(first) = path.nodes().first() else {
                return DocumentRect::default();
            };
            let (mut min_x, mut max_x) = (first.position.x, first.position.x);
            let (mut min_y, mut max_y) = (first.position.y, first.position.y);
            for node in path.nodes() {
                for point in [node.position, node.handle_in, node.handle_out] {
                    min_x = min_x.min(point.x);
                    max_x = max_x.max(point.x);
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                }
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

fn transformed_path(
    path: &BezierPath,
    transform: impl Fn(DocumentPoint) -> DocumentPoint,
) -> BezierPath {
    let mut nodes = path.nodes().iter().copied();
    let Some(first) = nodes.next() else {
        return BezierPath::default();
    };
    let mut transformed = BezierPath::new(BezierNode {
        position: transform(first.position),
        handle_in: transform(first.handle_in),
        handle_out: transform(first.handle_out),
    });
    for node in nodes {
        transformed.push_node(BezierNode {
            position: transform(node.position),
            handle_in: transform(node.handle_in),
            handle_out: transform(node.handle_out),
        });
    }
    if path.is_closed() {
        transformed.close();
    }
    transformed
}

fn edited_path_kind(
    original: &ObjectKind,
    node_index: usize,
    component: NodeComponent,
    point: DocumentPoint,
) -> ObjectKind {
    let ObjectKind::Path { path } = original else {
        return original.clone();
    };
    let mut path = path.clone();
    path.edit_node(node_index, component, point);
    ObjectKind::Path { path }
}

fn scale_axis(value: f32, old_start: f32, old_size: f32, new_start: f32, new_size: f32) -> f32 {
    if old_size.abs() <= f32::EPSILON {
        new_start
    } else {
        new_start + (value - old_start) / old_size * new_size
    }
}
