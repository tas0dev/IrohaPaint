use std::collections::{HashMap, HashSet};

use crate::document::{BezierNode, BezierPath, Document, DocumentPoint, ObjectKind};

const VERTEX_SNAP: f32 = 0.2;
const GAP_TOLERANCE: f32 = 1.25;
const INTERSECTION_EPSILON: f32 = 0.0001;

#[derive(Clone, Copy)]
struct Segment {
    start: DocumentPoint,
    end: DocumentPoint,
}

pub fn region_at(document: &Document, point: DocumentPoint) -> Option<BezierPath> {
    let segments = boundary_segments(document);
    if segments.is_empty() {
        return None;
    }
    let split = split_at_intersections(&segments);
    let split = close_small_gaps(split);
    let graph = Graph::from_segments(&split);
    let mut regions = graph
        .faces()
        .into_iter()
        .filter(|polygon| polygon.len() >= 3 && contains(polygon, point))
        .collect::<Vec<_>>();
    regions.sort_by(|first, second| {
        signed_area(first)
            .abs()
            .total_cmp(&signed_area(second).abs())
    });
    let polygon = regions.into_iter().next()?;
    let mut path = BezierPath::from_nodes(
        polygon
            .into_iter()
            .map(BezierNode::corner)
            .collect::<Vec<_>>(),
    )?;
    path.close();
    Some(path)
}

fn boundary_segments(document: &Document) -> Vec<Segment> {
    let mut segments = Vec::new();
    for layer in document
        .layers()
        .iter()
        .filter(|layer| layer.is_visible() && !layer.is_clipped())
    {
        for object in layer.objects() {
            match object.kind() {
                ObjectKind::Path {
                    path,
                    style,
                    cutouts,
                    ..
                } if visible_boundary(*style, path.is_closed()) => {
                    flatten_path(path, &mut segments);
                    for cutout in cutouts {
                        flatten_path(cutout, &mut segments);
                    }
                }
                ObjectKind::Rectangle { bounds, style } if visible_boundary(*style, true) => {
                    let points = [
                        DocumentPoint::new(bounds.x, bounds.y),
                        DocumentPoint::new(bounds.x + bounds.width, bounds.y),
                        DocumentPoint::new(bounds.x + bounds.width, bounds.y + bounds.height),
                        DocumentPoint::new(bounds.x, bounds.y + bounds.height),
                    ];
                    add_polyline(&points, true, &mut segments);
                }
                ObjectKind::Ellipse { bounds, style } if visible_boundary(*style, true) => {
                    let center = DocumentPoint::new(
                        bounds.x + bounds.width * 0.5,
                        bounds.y + bounds.height * 0.5,
                    );
                    let points = (0..64)
                        .map(|index| {
                            let angle = std::f32::consts::TAU * index as f32 / 64.0;
                            DocumentPoint::new(
                                center.x + angle.cos() * bounds.width * 0.5,
                                center.y + angle.sin() * bounds.height * 0.5,
                            )
                        })
                        .collect::<Vec<_>>();
                    add_polyline(&points, true, &mut segments);
                }
                _ => {}
            }
        }
    }
    segments
}

fn visible_boundary(style: crate::document::ObjectStyle, closed: bool) -> bool {
    (style.stroke.color.alpha > 0 && style.stroke.width > 0.0) || (closed && style.fill.alpha > 0)
}

fn flatten_path(path: &BezierPath, output: &mut Vec<Segment>) {
    let nodes = path.nodes();
    if nodes.len() < 2 {
        return;
    }
    let count = if path.is_closed() {
        nodes.len()
    } else {
        nodes.len() - 1
    };
    for index in 0..count {
        let start = nodes[index];
        let end = nodes[(index + 1) % nodes.len()];
        let length = distance(start.position, start.handle_out)
            + distance(start.handle_out, end.handle_in)
            + distance(end.handle_in, end.position);
        let steps = (length / 4.0).ceil() as usize;
        let steps = steps.clamp(8, 64);
        let mut previous = start.position;
        for step in 1..=steps {
            let current = cubic_point(start, end, step as f32 / steps as f32);
            if distance(previous, current) > INTERSECTION_EPSILON {
                output.push(Segment {
                    start: previous,
                    end: current,
                });
            }
            previous = current;
        }
    }
}

fn add_polyline(points: &[DocumentPoint], closed: bool, output: &mut Vec<Segment>) {
    for pair in points.windows(2) {
        output.push(Segment {
            start: pair[0],
            end: pair[1],
        });
    }
    if closed && points.len() > 2 {
        output.push(Segment {
            start: *points.last().expect("a closed polyline has points"),
            end: points[0],
        });
    }
}

fn split_at_intersections(segments: &[Segment]) -> Vec<Segment> {
    let mut parameters = vec![vec![0.0_f32, 1.0]; segments.len()];
    for first_index in 0..segments.len() {
        for second_index in first_index + 1..segments.len() {
            let first = segments[first_index];
            let second = segments[second_index];
            if !bounds_overlap(first, second) {
                continue;
            }
            if let Some((first_t, second_t)) = intersection(first, second) {
                parameters[first_index].push(first_t);
                parameters[second_index].push(second_t);
            }
        }
    }

    let mut output = Vec::new();
    for (segment, mut values) in segments.iter().copied().zip(parameters) {
        values.sort_by(f32::total_cmp);
        values.dedup_by(|first, second| (*first - *second).abs() <= INTERSECTION_EPSILON);
        for pair in values.windows(2) {
            if pair[1] - pair[0] <= INTERSECTION_EPSILON {
                continue;
            }
            output.push(Segment {
                start: lerp_point(segment.start, segment.end, pair[0]),
                end: lerp_point(segment.start, segment.end, pair[1]),
            });
        }
    }
    output
}

fn close_small_gaps(mut segments: Vec<Segment>) -> Vec<Segment> {
    let graph = Graph::from_segments(&segments);
    let endpoints = graph
        .neighbors
        .iter()
        .enumerate()
        .filter_map(|(index, neighbors)| (neighbors.len() == 1).then_some(index))
        .collect::<Vec<_>>();
    let mut connectors = Vec::new();

    for &endpoint in &endpoints {
        let point = graph.points[endpoint];
        let endpoint_target = endpoints
            .iter()
            .copied()
            .filter(|candidate| *candidate != endpoint)
            .map(|candidate| (candidate, distance(point, graph.points[candidate])))
            .filter(|(_, gap)| *gap > VERTEX_SNAP && *gap <= GAP_TOLERANCE)
            .min_by(|first, second| first.1.total_cmp(&second.1));

        if let Some((target, _)) = endpoint_target {
            connectors.push(Segment {
                start: point,
                end: graph.points[target],
            });
            continue;
        }

        let segment_target = segments
            .iter()
            .filter_map(|segment| {
                if distance(point, segment.start) <= VERTEX_SNAP
                    || distance(point, segment.end) <= VERTEX_SNAP
                {
                    return None;
                }
                let (projection, amount) = project_to_segment(point, *segment);
                (amount > INTERSECTION_EPSILON && amount < 1.0 - INTERSECTION_EPSILON)
                    .then_some((projection, distance(point, projection)))
            })
            .filter(|(_, gap)| *gap <= GAP_TOLERANCE)
            .min_by(|first, second| first.1.total_cmp(&second.1));
        if let Some((target, gap)) = segment_target
            && gap > VERTEX_SNAP
        {
            connectors.push(Segment {
                start: point,
                end: target,
            });
        }
    }

    if connectors.is_empty() {
        return segments;
    }
    segments.extend(connectors);
    split_at_intersections(&segments)
}

fn project_to_segment(point: DocumentPoint, segment: Segment) -> (DocumentPoint, f32) {
    let delta = subtract(segment.end, segment.start);
    let length_squared = delta.x * delta.x + delta.y * delta.y;
    if length_squared <= f32::EPSILON {
        return (segment.start, 0.0);
    }
    let from_start = subtract(point, segment.start);
    let amount =
        ((from_start.x * delta.x + from_start.y * delta.y) / length_squared).clamp(0.0, 1.0);
    (lerp_point(segment.start, segment.end, amount), amount)
}

fn intersection(first: Segment, second: Segment) -> Option<(f32, f32)> {
    let first_delta = subtract(first.end, first.start);
    let second_delta = subtract(second.end, second.start);
    let denominator = cross(first_delta, second_delta);
    if denominator.abs() <= INTERSECTION_EPSILON {
        return None;
    }
    let between = subtract(second.start, first.start);
    let first_t = cross(between, second_delta) / denominator;
    let second_t = cross(between, first_delta) / denominator;
    if (-INTERSECTION_EPSILON..=1.0 + INTERSECTION_EPSILON).contains(&first_t)
        && (-INTERSECTION_EPSILON..=1.0 + INTERSECTION_EPSILON).contains(&second_t)
    {
        Some((first_t.clamp(0.0, 1.0), second_t.clamp(0.0, 1.0)))
    } else {
        None
    }
}

fn bounds_overlap(first: Segment, second: Segment) -> bool {
    first.start.x.min(first.end.x) <= second.start.x.max(second.end.x) + INTERSECTION_EPSILON
        && first.start.x.max(first.end.x) + INTERSECTION_EPSILON >= second.start.x.min(second.end.x)
        && first.start.y.min(first.end.y) <= second.start.y.max(second.end.y) + INTERSECTION_EPSILON
        && first.start.y.max(first.end.y) + INTERSECTION_EPSILON >= second.start.y.min(second.end.y)
}

struct Graph {
    points: Vec<DocumentPoint>,
    neighbors: Vec<Vec<usize>>,
}

impl Graph {
    fn from_segments(segments: &[Segment]) -> Self {
        let mut points = Vec::new();
        let mut buckets = HashMap::<(i32, i32), Vec<usize>>::new();
        let mut edges = HashSet::new();
        for segment in segments {
            let start = vertex(&mut points, &mut buckets, segment.start);
            let end = vertex(&mut points, &mut buckets, segment.end);
            if start != end {
                edges.insert((start.min(end), start.max(end)));
            }
        }
        let mut neighbors = vec![Vec::new(); points.len()];
        for (first, second) in edges {
            neighbors[first].push(second);
            neighbors[second].push(first);
        }
        for (index, connected) in neighbors.iter_mut().enumerate() {
            connected.sort_by(|first, second| {
                angle(points[index], points[*first])
                    .total_cmp(&angle(points[index], points[*second]))
            });
        }
        Self { points, neighbors }
    }

    fn faces(&self) -> Vec<Vec<DocumentPoint>> {
        let mut visited = HashSet::<(usize, usize)>::new();
        let mut faces = Vec::new();
        for from in 0..self.points.len() {
            for &to in &self.neighbors[from] {
                if visited.contains(&(from, to)) {
                    continue;
                }
                let start = (from, to);
                let mut edge = start;
                let mut polygon = Vec::new();
                for _ in 0..self.points.len().saturating_mul(2).max(1) {
                    if !visited.insert(edge) {
                        break;
                    }
                    polygon.push(self.points[edge.0]);
                    let connected = &self.neighbors[edge.1];
                    let Some(reverse_index) = connected.iter().position(|next| *next == edge.0)
                    else {
                        break;
                    };
                    let next_index = (reverse_index + connected.len() - 1) % connected.len();
                    edge = (edge.1, connected[next_index]);
                    if edge == start {
                        if polygon.len() >= 3 && signed_area(&polygon).abs() > 0.01 {
                            faces.push(polygon);
                        }
                        break;
                    }
                }
            }
        }
        faces
    }
}

fn vertex(
    points: &mut Vec<DocumentPoint>,
    buckets: &mut HashMap<(i32, i32), Vec<usize>>,
    point: DocumentPoint,
) -> usize {
    let cell = (
        (point.x / VERTEX_SNAP).round() as i32,
        (point.y / VERTEX_SNAP).round() as i32,
    );
    for x in cell.0 - 1..=cell.0 + 1 {
        for y in cell.1 - 1..=cell.1 + 1 {
            if let Some(indices) = buckets.get(&(x, y)) {
                for &index in indices {
                    if distance(points[index], point) <= VERTEX_SNAP {
                        return index;
                    }
                }
            }
        }
    }
    let index = points.len();
    points.push(point);
    buckets.entry(cell).or_default().push(index);
    index
}

fn contains(polygon: &[DocumentPoint], point: DocumentPoint) -> bool {
    let mut inside = false;
    for index in 0..polygon.len() {
        let first = polygon[index];
        let second = polygon[(index + 1) % polygon.len()];
        if (first.y > point.y) != (second.y > point.y)
            && point.x < (second.x - first.x) * (point.y - first.y) / (second.y - first.y) + first.x
        {
            inside = !inside;
        }
    }
    inside
}

fn signed_area(polygon: &[DocumentPoint]) -> f32 {
    (0..polygon.len())
        .map(|index| {
            let first = polygon[index];
            let second = polygon[(index + 1) % polygon.len()];
            first.x * second.y - second.x * first.y
        })
        .sum::<f32>()
        * 0.5
}

fn cubic_point(start: BezierNode, end: BezierNode, t: f32) -> DocumentPoint {
    let inverse = 1.0 - t;
    let a = inverse * inverse * inverse;
    let b = 3.0 * inverse * inverse * t;
    let c = 3.0 * inverse * t * t;
    let d = t * t * t;
    DocumentPoint::new(
        start.position.x * a + start.handle_out.x * b + end.handle_in.x * c + end.position.x * d,
        start.position.y * a + start.handle_out.y * b + end.handle_in.y * c + end.position.y * d,
    )
}

fn angle(from: DocumentPoint, to: DocumentPoint) -> f32 {
    (to.y - from.y).atan2(to.x - from.x)
}

fn distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    let delta = subtract(second, first);
    (delta.x * delta.x + delta.y * delta.y).sqrt()
}

fn subtract(first: DocumentPoint, second: DocumentPoint) -> DocumentPoint {
    DocumentPoint::new(first.x - second.x, first.y - second.y)
}

fn cross(first: DocumentPoint, second: DocumentPoint) -> f32 {
    first.x * second.y - first.y * second.x
}

fn lerp_point(first: DocumentPoint, second: DocumentPoint, t: f32) -> DocumentPoint {
    DocumentPoint::new(
        first.x + (second.x - first.x) * t,
        first.y + (second.y - first.y) * t,
    )
}
