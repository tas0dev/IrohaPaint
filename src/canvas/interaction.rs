use viewkit::prelude::Point;

use crate::brush::BrushDefinition;
use crate::document::{
    BezierNode, BezierPath, DocumentPoint, DocumentRect, NodeComponent, ObjectId, ObjectKind,
    ObjectStyle, PaintDab,
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

pub const ROTATE_HANDLE_OFFSET: f32 = 28.0;

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
        style: ObjectStyle,
    },
    DrawingPencil {
        raw_points: Vec<DocumentPoint>,
        raw_pressures: Vec<f32>,
        preview: Option<BezierPath>,
        brush: BrushDefinition,
    },
    Painting {
        last_input: Option<DocumentPoint>,
        distance_since_dab: f32,
        spacing: f32,
        dab: PaintDab,
    },
    ErasingObjects {
        last: DocumentPoint,
        started: bool,
    },
    ErasingPathSections {
        last: DocumentPoint,
        started: bool,
        radius: f32,
    },
    DrawingBlob {
        raw_points: Vec<DocumentPoint>,
        raw_pressures: Vec<f32>,
        preview: Option<BezierPath>,
        style: ObjectStyle,
        smoothing: f32,
        streamline: f32,
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
        node_indices: Vec<usize>,
        component: NodeComponent,
        start: DocumentPoint,
        current: DocumentPoint,
        independent: bool,
    },
    SelectingNodes {
        id: ObjectId,
        start: DocumentPoint,
        current: DocumentPoint,
        additive: bool,
    },
    SelectingObjects {
        start: DocumentPoint,
        current: DocumentPoint,
        additive: bool,
    },
    Panning {
        start_canvas: Point,
        start_pan: Point,
    },
    MovingObjects {
        originals: Vec<(ObjectId, ObjectKind)>,
        start: DocumentPoint,
        current: DocumentPoint,
    },
    ResizingObjects {
        originals: Vec<(ObjectId, ObjectKind)>,
        original_bounds: DocumentRect,
        anchor: DocumentPoint,
        current: DocumentPoint,
        handle: ResizeHandle,
    },
    RotatingObjects {
        originals: Vec<(ObjectId, ObjectKind)>,
        center: DocumentPoint,
        start_angle: f32,
        current_angle: f32,
    },
}

impl Interaction {
    pub fn preview_kind(&self, object_id: ObjectId) -> Option<ObjectKind> {
        match self {
            Self::MovingObjects {
                originals,
                start,
                current,
            } => originals.iter().find_map(|(id, original)| {
                (*id == object_id).then(|| {
                    let delta = DocumentPoint::new(current.x - start.x, current.y - start.y);
                    translated_kind(original, delta)
                })
            }),
            Self::ResizingObjects {
                originals,
                original_bounds,
                current,
                anchor,
                ..
            } => originals.iter().find_map(|(id, original)| {
                (*id == object_id).then(|| {
                    group_resized_kind(
                        original,
                        *original_bounds,
                        DocumentRect::from_points(*anchor, *current),
                    )
                })
            }),
            Self::RotatingObjects {
                originals,
                center,
                start_angle,
                current_angle,
            } => originals.iter().find_map(|(id, original)| {
                (*id == object_id)
                    .then(|| rotated_kind(original, *center, *current_angle - *start_angle))
            }),
            Self::EditingPathNode {
                id,
                original,
                node_index,
                node_indices,
                component,
                start,
                current,
                independent,
            } if *id == object_id => Some(edited_path_kind(
                original,
                *node_index,
                node_indices,
                *component,
                *start,
                *current,
                *independent,
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
    let ObjectKind::Path {
        path,
        style,
        variable_width,
        cutouts,
    } = original
    else {
        return original.clone();
    };
    let mut path = path.clone();
    path.push_node(BezierNode::smooth(position, handle_out));
    ObjectKind::Path {
        path,
        style: *style,
        variable_width: *variable_width,
        cutouts: cutouts.clone(),
    }
}

pub(crate) fn translated_kind(kind: &ObjectKind, delta: DocumentPoint) -> ObjectKind {
    match kind {
        ObjectKind::Rectangle { bounds, style } => ObjectKind::Rectangle {
            bounds: bounds.translated(delta),
            style: *style,
        },
        ObjectKind::Ellipse { bounds, style } => ObjectKind::Ellipse {
            bounds: bounds.translated(delta),
            style: *style,
        },
        ObjectKind::Path {
            path,
            style,
            variable_width,
            cutouts,
        } => ObjectKind::Path {
            path: transformed_path(path, |point| {
                DocumentPoint::new(point.x + delta.x, point.y + delta.y)
            }),
            style: *style,
            variable_width: *variable_width,
            cutouts: cutouts
                .iter()
                .map(|path| {
                    transformed_path(path, |point| {
                        DocumentPoint::new(point.x + delta.x, point.y + delta.y)
                    })
                })
                .collect(),
        },
    }
}

fn resized_kind(kind: &ObjectKind, new_bounds: DocumentRect) -> ObjectKind {
    let old_bounds = kind_bounds(kind);
    match kind {
        ObjectKind::Rectangle { style, .. } => ObjectKind::Rectangle {
            bounds: new_bounds,
            style: *style,
        },
        ObjectKind::Ellipse { style, .. } => ObjectKind::Ellipse {
            bounds: new_bounds,
            style: *style,
        },
        ObjectKind::Path {
            path,
            style,
            variable_width,
            cutouts,
        } => ObjectKind::Path {
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
            style: *style,
            variable_width: *variable_width,
            cutouts: cutouts
                .iter()
                .map(|path| {
                    transformed_path(path, |point| {
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
                })
                .collect(),
        },
    }
}

pub(crate) fn group_resized_kind(
    kind: &ObjectKind,
    group_bounds: DocumentRect,
    new_group_bounds: DocumentRect,
) -> ObjectKind {
    let bounds = kind_bounds(kind);
    let new_bounds = DocumentRect {
        x: scale_axis(
            bounds.x,
            group_bounds.x,
            group_bounds.width,
            new_group_bounds.x,
            new_group_bounds.width,
        ),
        y: scale_axis(
            bounds.y,
            group_bounds.y,
            group_bounds.height,
            new_group_bounds.y,
            new_group_bounds.height,
        ),
        width: if group_bounds.width.abs() <= f32::EPSILON {
            bounds.width
        } else {
            bounds.width / group_bounds.width * new_group_bounds.width
        },
        height: if group_bounds.height.abs() <= f32::EPSILON {
            bounds.height
        } else {
            bounds.height / group_bounds.height * new_group_bounds.height
        },
    };
    resized_kind(kind, new_bounds)
}

pub fn rotated_kind(kind: &ObjectKind, center: DocumentPoint, angle: f32) -> ObjectKind {
    let transform = |point: DocumentPoint| {
        let cosine = angle.cos();
        let sine = angle.sin();
        let x = point.x - center.x;
        let y = point.y - center.y;
        DocumentPoint::new(
            center.x + x * cosine - y * sine,
            center.y + x * sine + y * cosine,
        )
    };
    match kind {
        ObjectKind::Rectangle { bounds, style } => ObjectKind::Path {
            path: transformed_path(&rectangle_path(*bounds), transform),
            style: *style,
            variable_width: false,
            cutouts: Vec::new(),
        },
        ObjectKind::Ellipse { bounds, style } => ObjectKind::Path {
            path: transformed_path(&ellipse_path(*bounds), transform),
            style: *style,
            variable_width: false,
            cutouts: Vec::new(),
        },
        ObjectKind::Path {
            path,
            style,
            variable_width,
            cutouts,
        } => ObjectKind::Path {
            path: transformed_path(path, transform),
            style: *style,
            variable_width: *variable_width,
            cutouts: cutouts
                .iter()
                .map(|path| transformed_path(path, transform))
                .collect(),
        },
    }
}

pub(crate) fn flipped_kind(
    kind: &ObjectKind,
    center: DocumentPoint,
    horizontal: bool,
) -> ObjectKind {
    let transform = |point: DocumentPoint| {
        if horizontal {
            DocumentPoint::new(center.x * 2.0 - point.x, point.y)
        } else {
            DocumentPoint::new(point.x, center.y * 2.0 - point.y)
        }
    };
    match kind {
        ObjectKind::Rectangle { bounds, style } => ObjectKind::Rectangle {
            bounds: DocumentRect {
                x: if horizontal {
                    center.x * 2.0 - bounds.x - bounds.width
                } else {
                    bounds.x
                },
                y: if horizontal {
                    bounds.y
                } else {
                    center.y * 2.0 - bounds.y - bounds.height
                },
                ..*bounds
            },
            style: *style,
        },
        ObjectKind::Ellipse { bounds, style } => ObjectKind::Ellipse {
            bounds: DocumentRect {
                x: if horizontal {
                    center.x * 2.0 - bounds.x - bounds.width
                } else {
                    bounds.x
                },
                y: if horizontal {
                    bounds.y
                } else {
                    center.y * 2.0 - bounds.y - bounds.height
                },
                ..*bounds
            },
            style: *style,
        },
        ObjectKind::Path {
            path,
            style,
            variable_width,
            cutouts,
        } => ObjectKind::Path {
            path: transformed_path(path, transform),
            style: *style,
            variable_width: *variable_width,
            cutouts: cutouts
                .iter()
                .map(|path| transformed_path(path, transform))
                .collect(),
        },
    }
}

fn rectangle_path(bounds: DocumentRect) -> BezierPath {
    let points = [
        DocumentPoint::new(bounds.x, bounds.y),
        DocumentPoint::new(bounds.x + bounds.width, bounds.y),
        DocumentPoint::new(bounds.x + bounds.width, bounds.y + bounds.height),
        DocumentPoint::new(bounds.x, bounds.y + bounds.height),
    ];
    closed_path(&points.map(BezierNode::corner))
}

fn ellipse_path(bounds: DocumentRect) -> BezierPath {
    const KAPPA: f32 = 0.552_284_8;
    let center = DocumentPoint::new(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    let rx = bounds.width * 0.5;
    let ry = bounds.height * 0.5;
    closed_path(&[
        BezierNode {
            position: DocumentPoint::new(center.x + rx, center.y),
            handle_in: DocumentPoint::new(center.x + rx, center.y - ry * KAPPA),
            handle_out: DocumentPoint::new(center.x + rx, center.y + ry * KAPPA),
            kind: crate::document::NodeKind::Smooth,
            width: 1.0,
        },
        BezierNode {
            position: DocumentPoint::new(center.x, center.y + ry),
            handle_in: DocumentPoint::new(center.x + rx * KAPPA, center.y + ry),
            handle_out: DocumentPoint::new(center.x - rx * KAPPA, center.y + ry),
            kind: crate::document::NodeKind::Smooth,
            width: 1.0,
        },
        BezierNode {
            position: DocumentPoint::new(center.x - rx, center.y),
            handle_in: DocumentPoint::new(center.x - rx, center.y + ry * KAPPA),
            handle_out: DocumentPoint::new(center.x - rx, center.y - ry * KAPPA),
            kind: crate::document::NodeKind::Smooth,
            width: 1.0,
        },
        BezierNode {
            position: DocumentPoint::new(center.x, center.y - ry),
            handle_in: DocumentPoint::new(center.x - rx * KAPPA, center.y - ry),
            handle_out: DocumentPoint::new(center.x + rx * KAPPA, center.y - ry),
            kind: crate::document::NodeKind::Smooth,
            width: 1.0,
        },
    ])
}

fn closed_path(nodes: &[BezierNode]) -> BezierPath {
    let mut path = BezierPath::new(nodes[0]);
    for node in &nodes[1..] {
        path.push_node(*node);
    }
    path.close();
    path
}

pub fn kind_bounds(kind: &ObjectKind) -> DocumentRect {
    match kind {
        ObjectKind::Rectangle { bounds, .. } | ObjectKind::Ellipse { bounds, .. } => *bounds,
        ObjectKind::Path { path, .. } => {
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
        kind: first.kind,
        width: first.width,
    });
    for node in nodes {
        transformed.push_node(BezierNode {
            position: transform(node.position),
            handle_in: transform(node.handle_in),
            handle_out: transform(node.handle_out),
            kind: node.kind,
            width: node.width,
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
    node_indices: &[usize],
    component: NodeComponent,
    start: DocumentPoint,
    point: DocumentPoint,
    independent: bool,
) -> ObjectKind {
    let ObjectKind::Path {
        path,
        style,
        variable_width,
        cutouts,
    } = original
    else {
        return original.clone();
    };
    let mut path = path.clone();
    if component == NodeComponent::Anchor {
        path.translate_nodes(
            node_indices,
            DocumentPoint::new(point.x - start.x, point.y - start.y),
        );
    } else {
        path.edit_node(node_index, component, point, independent);
    }
    ObjectKind::Path {
        path,
        style: *style,
        variable_width: *variable_width,
        cutouts: cutouts.clone(),
    }
}

fn scale_axis(value: f32, old_start: f32, old_size: f32, new_start: f32, new_size: f32) -> f32 {
    if old_size.abs() <= f32::EPSILON {
        new_start
    } else {
        new_start + (value - old_start) / old_size * new_size
    }
}
