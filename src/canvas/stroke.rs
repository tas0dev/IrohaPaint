use crate::document::{BezierNode, BezierPath, DocumentPoint};

pub fn fit_pencil_stroke(points: &[DocumentPoint], tolerance: f32) -> Option<BezierPath> {
    if points.len() < 2 {
        return None;
    }

    let tolerance = tolerance.max(0.01);
    let smoothed = smooth_points(points);
    let sampled = resample_points(&smoothed, tolerance * 0.5);
    let simplified = simplify_points(&sampled, tolerance);
    if simplified.len() < 2 {
        return None;
    }

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
            }
        })
        .collect();

    BezierPath::from_nodes(nodes)
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
