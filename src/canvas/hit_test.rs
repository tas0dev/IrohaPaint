use crate::document::{
    BezierPath, Document, DocumentPoint, DocumentRect, NodeComponent, ObjectId, ObjectKind,
};

use super::interaction::ResizeHandle;

pub fn object_at(document: &Document, point: DocumentPoint, tolerance: f32) -> Option<ObjectId> {
    document
        .layers()
        .iter()
        .rev()
        .flat_map(|layer| layer.objects().iter().rev())
        .find(|object| kind_contains(object.kind(), point, tolerance))
        .map(|object| object.id())
}

pub fn resize_handle_at(
    bounds: DocumentRect,
    point: DocumentPoint,
    tolerance: f32,
) -> Option<ResizeHandle> {
    ResizeHandle::ALL.into_iter().find(|handle| {
        let handle_point = handle.position(bounds);
        (point.x - handle_point.x).abs() <= tolerance
            && (point.y - handle_point.y).abs() <= tolerance
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeHit {
    pub index: usize,
    pub component: NodeComponent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentHit {
    pub start_index: usize,
    pub t: f32,
    pub point: DocumentPoint,
}

pub fn path_node_at(
    path: &BezierPath,
    selected_nodes: &[usize],
    point: DocumentPoint,
    tolerance: f32,
) -> Option<NodeHit> {
    for &index in selected_nodes {
        let Some(node) = path.nodes().get(index) else {
            continue;
        };
        for (component, handle) in [
            (NodeComponent::HandleIn, node.handle_in),
            (NodeComponent::HandleOut, node.handle_out),
        ] {
            if point_distance(node.position, handle) > f32::EPSILON
                && point_distance(point, handle) <= tolerance
            {
                return Some(NodeHit { index, component });
            }
        }
    }

    path.nodes()
        .iter()
        .enumerate()
        .find(|(_, node)| point_distance(point, node.position) <= tolerance)
        .map(|(index, _)| NodeHit {
            index,
            component: NodeComponent::Anchor,
        })
}

pub fn path_segment_at(
    path: &BezierPath,
    point: DocumentPoint,
    tolerance: f32,
) -> Option<SegmentHit> {
    let nodes = path.nodes();
    if nodes.len() < 2 {
        return None;
    }
    let segment_count = if path.is_closed() {
        nodes.len()
    } else {
        nodes.len() - 1
    };
    let mut closest = None;
    let mut closest_distance = tolerance;
    for start_index in 0..segment_count {
        let end_index = (start_index + 1) % nodes.len();
        let start = nodes[start_index];
        let end = nodes[end_index];
        const STEPS: usize = 32;
        let mut best_t = 0.0;
        let mut best_distance = f32::MAX;
        for step in 0..=STEPS {
            let t = step as f32 / STEPS as f32;
            let curve_point = cubic_point(
                start.position,
                start.handle_out,
                end.handle_in,
                end.position,
                t,
            );
            let distance = point_distance(point, curve_point);
            if distance < best_distance {
                best_distance = distance;
                best_t = t;
            }
        }
        let radius = 1.0 / STEPS as f32;
        let mut low = (best_t - radius).max(0.0);
        let mut high = (best_t + radius).min(1.0);
        for _ in 0..8 {
            let left = low + (high - low) / 3.0;
            let right = high - (high - low) / 3.0;
            let left_distance = point_distance(
                point,
                cubic_point(
                    start.position,
                    start.handle_out,
                    end.handle_in,
                    end.position,
                    left,
                ),
            );
            let right_distance = point_distance(
                point,
                cubic_point(
                    start.position,
                    start.handle_out,
                    end.handle_in,
                    end.position,
                    right,
                ),
            );
            if left_distance <= right_distance {
                high = right;
            } else {
                low = left;
            }
        }
        let t = (low + high) / 2.0;
        let curve_point = cubic_point(
            start.position,
            start.handle_out,
            end.handle_in,
            end.position,
            t,
        );
        let distance = point_distance(point, curve_point);
        if distance <= closest_distance {
            closest_distance = distance;
            closest = Some(SegmentHit {
                start_index,
                t,
                point: curve_point,
            });
        }
    }
    closest
}

pub fn is_first_path_node(path: &BezierPath, point: DocumentPoint, tolerance: f32) -> bool {
    path.nodes()
        .first()
        .is_some_and(|node| point_distance(point, node.position) <= tolerance)
}

fn kind_contains(kind: &ObjectKind, point: DocumentPoint, tolerance: f32) -> bool {
    match kind {
        ObjectKind::Rectangle { bounds } => expanded(*bounds, tolerance).contains(point),
        ObjectKind::Ellipse { bounds } => ellipse_contains(*bounds, point, tolerance),
        ObjectKind::Path { path, stroke } => {
            bezier_path_contains(path, point, tolerance + stroke.width / 2.0)
        }
    }
}

fn bezier_path_contains(path: &BezierPath, point: DocumentPoint, tolerance: f32) -> bool {
    let nodes = path.nodes();
    if nodes.len() < 2 {
        return nodes
            .first()
            .is_some_and(|node| point_distance(point, node.position) <= tolerance);
    }

    let open_segments = nodes.windows(2).any(|nodes| {
        cubic_contains(
            point,
            nodes[0].position,
            nodes[0].handle_out,
            nodes[1].handle_in,
            nodes[1].position,
            tolerance,
        )
    });
    if open_segments || !path.is_closed() {
        return open_segments;
    }

    let first = nodes.first().expect("a closed path has nodes");
    let last = nodes.last().expect("a closed path has nodes");
    cubic_contains(
        point,
        last.position,
        last.handle_out,
        first.handle_in,
        first.position,
        tolerance,
    )
}

fn cubic_contains(
    point: DocumentPoint,
    start: DocumentPoint,
    control_1: DocumentPoint,
    control_2: DocumentPoint,
    end: DocumentPoint,
    tolerance: f32,
) -> bool {
    const STEPS: usize = 24;
    let mut previous = start;
    for step in 1..=STEPS {
        let t = step as f32 / STEPS as f32;
        let current = cubic_point(start, control_1, control_2, end, t);
        if distance_to_segment(point, previous, current) <= tolerance {
            return true;
        }
        previous = current;
    }
    false
}

fn cubic_point(
    start: DocumentPoint,
    control_1: DocumentPoint,
    control_2: DocumentPoint,
    end: DocumentPoint,
    t: f32,
) -> DocumentPoint {
    let inverse = 1.0 - t;
    let start_weight = inverse * inverse * inverse;
    let first_weight = 3.0 * inverse * inverse * t;
    let second_weight = 3.0 * inverse * t * t;
    let end_weight = t * t * t;
    DocumentPoint::new(
        start.x * start_weight
            + control_1.x * first_weight
            + control_2.x * second_weight
            + end.x * end_weight,
        start.y * start_weight
            + control_1.y * first_weight
            + control_2.y * second_weight
            + end.y * end_weight,
    )
}

fn point_distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    ((first.x - second.x).powi(2) + (first.y - second.y).powi(2)).sqrt()
}

fn expanded(bounds: DocumentRect, amount: f32) -> DocumentRect {
    DocumentRect {
        x: bounds.x - amount,
        y: bounds.y - amount,
        width: bounds.width + amount * 2.0,
        height: bounds.height + amount * 2.0,
    }
}

fn ellipse_contains(bounds: DocumentRect, point: DocumentPoint, tolerance: f32) -> bool {
    let bounds = expanded(bounds, tolerance);
    let radius_x = bounds.width / 2.0;
    let radius_y = bounds.height / 2.0;
    if radius_x <= f32::EPSILON || radius_y <= f32::EPSILON {
        return false;
    }

    let center_x = bounds.x + radius_x;
    let center_y = bounds.y + radius_y;
    let x = (point.x - center_x) / radius_x;
    let y = (point.y - center_y) / radius_y;
    x * x + y * y <= 1.0
}

fn distance_to_segment(point: DocumentPoint, start: DocumentPoint, end: DocumentPoint) -> f32 {
    let segment_x = end.x - start.x;
    let segment_y = end.y - start.y;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared <= f32::EPSILON {
        return ((point.x - start.x).powi(2) + (point.y - start.y).powi(2)).sqrt();
    }

    let projection =
        ((point.x - start.x) * segment_x + (point.y - start.y) * segment_y) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    let closest_x = start.x + segment_x * projection;
    let closest_y = start.y + segment_y * projection;
    ((point.x - closest_x).powi(2) + (point.y - closest_y).powi(2)).sqrt()
}
