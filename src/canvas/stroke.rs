use crate::document::{BezierNode, BezierPath, DocumentPoint, NodeKind};

const BLOB_CAP_SEGMENTS: usize = 6;
const BLOB_DOT_SEGMENTS: usize = 12;

pub fn fit_pencil_stroke(points: &[DocumentPoint], tolerance: f32) -> Option<BezierPath> {
    let first = *points.first()?;
    if points.len() == 1 {
        return BezierPath::from_nodes(vec![BezierNode::corner(first), BezierNode::corner(first)]);
    }
    let tolerance = tolerance.max(0.01);
    let smoothed = smooth_points(points);
    let sampled = resample_points(&smoothed, tolerance * 0.5);
    let simplified = simplify_points(&sampled, tolerance);
    if simplified.len() < 2 {
        return None;
    }
    let widths = velocity_widths(points, tolerance);

    let last_index = simplified.len() - 1;
    let nodes = simplified
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let (handle_in, handle_out) = if index == 0 {
                (*position, interpolate(*position, simplified[1], 1.0 / 3.0))
            } else if index == last_index {
                (
                    interpolate(*position, simplified[index - 1], 1.0 / 3.0),
                    *position,
                )
            } else {
                let previous = simplified[index - 1];
                let next = simplified[index + 1];
                let tangent =
                    DocumentPoint::new((next.x - previous.x) / 6.0, (next.y - previous.y) / 6.0);
                (
                    DocumentPoint::new(position.x - tangent.x, position.y - tangent.y),
                    DocumentPoint::new(position.x + tangent.x, position.y + tangent.y),
                )
            };

            BezierNode {
                position: *position,
                handle_in,
                handle_out,
                kind: if index == 0 || index == last_index {
                    NodeKind::Corner
                } else {
                    NodeKind::Smooth
                },
                width: nearest_width(*position, points, &widths),
            }
        })
        .collect();

    BezierPath::from_nodes(nodes)
}

pub fn fit_blob_stroke(points: &[DocumentPoint], width: f32, tolerance: f32) -> Option<BezierPath> {
    let radius = width.max(0.1) / 2.0;
    if points.len() == 1 {
        return closed_spline(
            (0..BLOB_DOT_SEGMENTS)
                .map(|index| {
                    let angle = std::f32::consts::TAU * index as f32 / BLOB_DOT_SEGMENTS as f32;
                    DocumentPoint::new(
                        points[0].x + angle.cos() * radius,
                        points[0].y + angle.sin() * radius,
                    )
                })
                .collect(),
        );
    }
    if points.len() < 2 {
        return None;
    }

    let tolerance = tolerance.max(0.01);
    let smoothed = smooth_points(points);
    let sampled = resample_points(&smoothed, tolerance * 0.5);
    let centers = simplify_points(&sampled, tolerance);
    if centers.len() < 2 {
        return None;
    }

    let normals = (0..centers.len())
        .map(|index| {
            let previous = centers[index.saturating_sub(1)];
            let next = centers[(index + 1).min(centers.len() - 1)];
            unit_normal(previous, next)
        })
        .collect::<Vec<_>>();
    let left = centers
        .iter()
        .zip(&normals)
        .map(|(point, normal)| {
            DocumentPoint::new(point.x + normal.x * radius, point.y + normal.y * radius)
        })
        .collect::<Vec<_>>();
    let right = centers
        .iter()
        .zip(&normals)
        .map(|(point, normal)| {
            DocumentPoint::new(point.x - normal.x * radius, point.y - normal.y * radius)
        })
        .collect::<Vec<_>>();

    let mut outline = left;
    let end = *centers.last()?;
    let end_normal = *normals.last()?;
    let end_angle = end_normal.y.atan2(end_normal.x);
    for index in 1..=BLOB_CAP_SEGMENTS {
        let angle = end_angle - std::f32::consts::PI * index as f32 / BLOB_CAP_SEGMENTS as f32;
        outline.push(DocumentPoint::new(
            end.x + angle.cos() * radius,
            end.y + angle.sin() * radius,
        ));
    }
    outline.extend(right.iter().rev().skip(1).copied());

    let start = centers[0];
    let start_normal = normals[0];
    let start_angle = (-start_normal.y).atan2(-start_normal.x);
    for index in 1..BLOB_CAP_SEGMENTS {
        let angle = start_angle - std::f32::consts::PI * index as f32 / BLOB_CAP_SEGMENTS as f32;
        outline.push(DocumentPoint::new(
            start.x + angle.cos() * radius,
            start.y + angle.sin() * radius,
        ));
    }

    closed_spline(outline)
}

fn closed_spline(points: Vec<DocumentPoint>) -> Option<BezierPath> {
    if points.len() < 3 {
        return None;
    }
    let count = points.len();
    let nodes = points
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let previous = points[(index + count - 1) % count];
            let next = points[(index + 1) % count];
            let tangent = unit_tangent(previous, next);
            let incoming = distance(previous, *position) / 3.0;
            let outgoing = distance(*position, next) / 3.0;
            BezierNode {
                position: *position,
                handle_in: DocumentPoint::new(
                    position.x - tangent.x * incoming,
                    position.y - tangent.y * incoming,
                ),
                handle_out: DocumentPoint::new(
                    position.x + tangent.x * outgoing,
                    position.y + tangent.y * outgoing,
                ),
                kind: NodeKind::Smooth,
                width: 1.0,
            }
        })
        .collect();
    let mut path = BezierPath::from_nodes(nodes)?;
    path.close();
    Some(path)
}

fn velocity_widths(points: &[DocumentPoint], tolerance: f32) -> Vec<f32> {
    let scale = tolerance.max(0.01) * 2.5;
    let raw = (0..points.len())
        .map(|index| {
            let previous = points[index.saturating_sub(1)];
            let next = points[(index + 1).min(points.len() - 1)];
            let velocity = distance(previous, next) / scale;
            (1.0 / (1.0 + velocity * 0.22)).clamp(0.12, 1.0)
        })
        .collect::<Vec<_>>();
    if raw.len() < 3 {
        return raw;
    }
    let mut smoothed = Vec::with_capacity(raw.len());
    smoothed.push(raw[0]);
    for window in raw.windows(3) {
        smoothed.push((window[0] + window[1] * 2.0 + window[2]) / 4.0);
    }
    smoothed.push(*raw.last().expect("a stroke has widths"));
    smoothed
}

fn nearest_width(point: DocumentPoint, samples: &[DocumentPoint], widths: &[f32]) -> f32 {
    samples
        .iter()
        .zip(widths)
        .min_by(|(first, _), (second, _)| {
            distance(point, **first).total_cmp(&distance(point, **second))
        })
        .map_or(1.0, |(_, width)| *width)
}

fn unit_normal(first: DocumentPoint, second: DocumentPoint) -> DocumentPoint {
    let tangent = unit_tangent(first, second);
    DocumentPoint::new(-tangent.y, tangent.x)
}

fn unit_tangent(first: DocumentPoint, second: DocumentPoint) -> DocumentPoint {
    let delta_x = second.x - first.x;
    let delta_y = second.y - first.y;
    let length = (delta_x * delta_x + delta_y * delta_y).sqrt();
    if length <= f32::EPSILON {
        DocumentPoint::new(1.0, 0.0)
    } else {
        DocumentPoint::new(delta_x / length, delta_y / length)
    }
}

fn smooth_points(points: &[DocumentPoint]) -> Vec<DocumentPoint> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut smoothed = Vec::with_capacity(points.len());
    smoothed.push(points[0]);
    for window in points.windows(3) {
        smoothed.push(DocumentPoint::new(
            (window[0].x + window[1].x * 2.0 + window[2].x) / 4.0,
            (window[0].y + window[1].y * 2.0 + window[2].y) / 4.0,
        ));
    }
    smoothed.push(*points.last().expect("a stroke has points"));
    smoothed
}

fn resample_points(points: &[DocumentPoint], spacing: f32) -> Vec<DocumentPoint> {
    let mut sampled = Vec::with_capacity(points.len());
    sampled.push(points[0]);
    for point in &points[1..points.len() - 1] {
        if distance(*sampled.last().expect("sampled starts non-empty"), *point) >= spacing {
            sampled.push(*point);
        }
    }

    let last = *points.last().expect("a stroke has points");
    if distance(*sampled.last().expect("sampled starts non-empty"), last) > f32::EPSILON {
        sampled.push(last);
    }
    sampled
}

fn simplify_points(points: &[DocumentPoint], tolerance: f32) -> Vec<DocumentPoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut ranges = vec![(0, points.len() - 1)];
    while let Some((start, end)) = ranges.pop() {
        if end <= start + 1 {
            continue;
        }

        let mut furthest_index = start;
        let mut furthest_distance = 0.0;
        for index in start + 1..end {
            let distance = distance_to_segment(points[index], points[start], points[end]);
            if distance > furthest_distance {
                furthest_distance = distance;
                furthest_index = index;
            }
        }

        if furthest_distance > tolerance {
            keep[furthest_index] = true;
            ranges.push((start, furthest_index));
            ranges.push((furthest_index, end));
        }
    }

    points
        .iter()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(*point))
        .collect()
}

fn interpolate(from: DocumentPoint, to: DocumentPoint, amount: f32) -> DocumentPoint {
    DocumentPoint::new(
        from.x + (to.x - from.x) * amount,
        from.y + (to.y - from.y) * amount,
    )
}

fn distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    ((second.x - first.x).powi(2) + (second.y - first.y).powi(2)).sqrt()
}

fn distance_to_segment(point: DocumentPoint, start: DocumentPoint, end: DocumentPoint) -> f32 {
    let segment_x = end.x - start.x;
    let segment_y = end.y - start.y;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared <= f32::EPSILON {
        return distance(point, start);
    }

    let projection =
        ((point.x - start.x) * segment_x + (point.y - start.y) * segment_y) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    let closest = DocumentPoint::new(
        start.x + segment_x * projection,
        start.y + segment_y * projection,
    );
    distance(point, closest)
}
