use super::{BezierNode, BezierPath, DocumentPoint, NodeKind, StrokeCap, StrokeStyle};

const CURVE_STEPS: usize = 10;
const CAP_STEPS: usize = 8;
const DOT_STEPS: usize = 16;

/// Builds transient filled contours from an editable centerline.
///
/// Open strokes produce one closed contour. Closed centerlines produce an
/// outer and inner contour which callers render with the even-odd fill rule.
pub(crate) fn variable_stroke_outlines(path: &BezierPath, stroke: StrokeStyle) -> Vec<BezierPath> {
    let samples = flatten(path);
    if samples.len() < 2 {
        return Vec::new();
    }
    if samples
        .windows(2)
        .all(|pair| distance(pair[0].position, pair[1].position) <= f32::EPSILON)
    {
        return dot_outline(samples[0].position, stroke)
            .into_iter()
            .collect();
    }

    let progress = arc_progress(&samples);
    let normals = sample_normals(&samples, path.is_closed());
    let radii = samples
        .iter()
        .zip(&progress)
        .zip(&normals)
        .map(|((sample, progress), normal)| {
            stroke.width.max(0.1)
                * 0.5
                * tapered_width(sample.width, *progress, path, stroke)
                * nib_extent(*normal, stroke)
        })
        .collect::<Vec<_>>();
    let mut left = offset_points(&samples, &normals, &radii, 1.0);
    let mut right = offset_points(&samples, &normals, &radii, -1.0);

    if path.is_closed() {
        let mut inner = right;
        inner.reverse();
        return [closed_spline(left), closed_spline(inner)]
            .into_iter()
            .flatten()
            .collect();
    }

    if stroke.cap == StrokeCap::Square {
        extend_square_caps(&samples, &radii, &mut left, &mut right);
    }

    let mut outline = left;
    if stroke.cap == StrokeCap::Round {
        append_round_cap(
            &mut outline,
            samples[samples.len() - 1].position,
            normals[normals.len() - 1],
            radii[radii.len() - 1],
            false,
        );
    } else {
        outline.push(*right.last().expect("a stroke has a right edge"));
    }
    outline.extend(right.iter().rev().skip(1).copied());
    if stroke.cap == StrokeCap::Round {
        append_round_cap(
            &mut outline,
            samples[0].position,
            normals[0],
            radii[0],
            true,
        );
    }
    closed_spline(outline).into_iter().collect()
}

fn dot_outline(center: DocumentPoint, stroke: StrokeStyle) -> Option<BezierPath> {
    let major = stroke.width.max(0.1) * 0.5;
    let minor = major * stroke.tip_roundness.clamp(0.05, 1.0);
    let cosine = stroke.tip_angle.cos();
    let sine = stroke.tip_angle.sin();
    closed_spline(
        (0..DOT_STEPS)
            .map(|step| {
                let angle = std::f32::consts::TAU * step as f32 / DOT_STEPS as f32;
                let x = angle.cos() * major;
                let y = angle.sin() * minor;
                DocumentPoint::new(
                    center.x + x * cosine - y * sine,
                    center.y + x * sine + y * cosine,
                )
            })
            .collect(),
    )
}

#[derive(Clone, Copy)]
struct Sample {
    position: DocumentPoint,
    width: f32,
}

fn flatten(path: &BezierPath) -> Vec<Sample> {
    let nodes = path.nodes();
    if nodes.len() < 2 {
        return Vec::new();
    }
    let segment_count = nodes.len() - 1 + usize::from(path.is_closed());
    let mut samples = Vec::with_capacity(segment_count * CURVE_STEPS + 1);
    for segment in 0..segment_count {
        let start = nodes[segment % nodes.len()];
        let end = nodes[(segment + 1) % nodes.len()];
        let first_step = usize::from(segment > 0);
        for step in first_step..=CURVE_STEPS {
            let t = step as f32 / CURVE_STEPS as f32;
            samples.push(Sample {
                position: cubic_point(
                    start.position,
                    start.handle_out,
                    end.handle_in,
                    end.position,
                    t,
                ),
                width: start.width + (end.width - start.width) * t,
            });
        }
    }
    if path.is_closed() && samples.len() > 1 {
        samples.pop();
    }
    samples
}

fn arc_progress(samples: &[Sample]) -> Vec<f32> {
    let mut progress = Vec::with_capacity(samples.len());
    progress.push(0.0);
    for pair in samples.windows(2) {
        progress.push(
            progress.last().copied().unwrap_or_default()
                + distance(pair[0].position, pair[1].position),
        );
    }
    let total = progress.last().copied().unwrap_or_default();
    if total > f32::EPSILON {
        for value in &mut progress {
            *value /= total;
        }
    }
    progress
}

fn tapered_width(value: f32, progress: f32, path: &BezierPath, stroke: StrokeStyle) -> f32 {
    let minimum = stroke.minimum_width.clamp(0.01, 1.0);
    let mut width = value.clamp(minimum, 1.0);
    if path.is_closed() {
        return width;
    }
    if stroke.taper_start > f32::EPSILON && progress < stroke.taper_start {
        let amount = (progress / stroke.taper_start).clamp(0.0, 1.0);
        width = minimum + (width - minimum) * amount;
    }
    if stroke.taper_end > f32::EPSILON && 1.0 - progress < stroke.taper_end {
        let amount = ((1.0 - progress) / stroke.taper_end).clamp(0.0, 1.0);
        width = minimum + (width - minimum) * amount;
    }
    width
}

fn nib_extent(normal: DocumentPoint, stroke: StrokeStyle) -> f32 {
    let roundness = stroke.tip_roundness.clamp(0.05, 1.0);
    let angle = normal.y.atan2(normal.x) - stroke.tip_angle;
    let cosine = angle.cos();
    let sine = angle.sin();
    roundness / ((roundness * cosine).powi(2) + sine.powi(2)).sqrt()
}

fn sample_normals(samples: &[Sample], closed: bool) -> Vec<DocumentPoint> {
    (0..samples.len())
        .map(|index| {
            let previous = if index > 0 {
                samples[index - 1].position
            } else if closed {
                samples[samples.len() - 1].position
            } else {
                samples[index].position
            };
            let next = if index + 1 < samples.len() {
                samples[index + 1].position
            } else if closed {
                samples[0].position
            } else {
                samples[index].position
            };
            unit_normal(previous, next)
        })
        .collect()
}

fn offset_points(
    samples: &[Sample],
    normals: &[DocumentPoint],
    radii: &[f32],
    direction: f32,
) -> Vec<DocumentPoint> {
    samples
        .iter()
        .zip(normals)
        .zip(radii)
        .map(|((sample, normal), radius)| {
            DocumentPoint::new(
                sample.position.x + normal.x * radius * direction,
                sample.position.y + normal.y * radius * direction,
            )
        })
        .collect()
}

fn append_round_cap(
    outline: &mut Vec<DocumentPoint>,
    center: DocumentPoint,
    normal: DocumentPoint,
    radius: f32,
    start: bool,
) {
    let angle = normal.y.atan2(normal.x);
    for step in 1..=CAP_STEPS {
        if start && step == CAP_STEPS {
            break;
        }
        let amount = step as f32 / CAP_STEPS as f32;
        let angle = if start {
            angle + std::f32::consts::PI * (1.0 + amount)
        } else {
            angle - std::f32::consts::PI * amount
        };
        outline.push(DocumentPoint::new(
            center.x + angle.cos() * radius,
            center.y + angle.sin() * radius,
        ));
    }
}

fn extend_square_caps(
    samples: &[Sample],
    radii: &[f32],
    left: &mut [DocumentPoint],
    right: &mut [DocumentPoint],
) {
    let start_tangent = unit_tangent(samples[0].position, samples[1].position);
    let last = samples.len() - 1;
    let end_tangent = unit_tangent(samples[last - 1].position, samples[last].position);
    for point in [&mut left[0], &mut right[0]] {
        point.x -= start_tangent.x * radii[0];
        point.y -= start_tangent.y * radii[0];
    }
    for point in [&mut left[last], &mut right[last]] {
        point.x += end_tangent.x * radii[last];
        point.y += end_tangent.y * radii[last];
    }
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
            let tangent =
                DocumentPoint::new((next.x - previous.x) / 6.0, (next.y - previous.y) / 6.0);
            BezierNode {
                position: *position,
                handle_in: DocumentPoint::new(position.x - tangent.x, position.y - tangent.y),
                handle_out: DocumentPoint::new(position.x + tangent.x, position.y + tangent.y),
                kind: NodeKind::Smooth,
                width: 1.0,
            }
        })
        .collect();
    let mut path = BezierPath::from_nodes(nodes)?;
    path.close();
    Some(path)
}

fn cubic_point(
    start: DocumentPoint,
    first: DocumentPoint,
    second: DocumentPoint,
    end: DocumentPoint,
    t: f32,
) -> DocumentPoint {
    let inverse = 1.0 - t;
    DocumentPoint::new(
        start.x * inverse.powi(3)
            + first.x * 3.0 * inverse.powi(2) * t
            + second.x * 3.0 * inverse * t.powi(2)
            + end.x * t.powi(3),
        start.y * inverse.powi(3)
            + first.y * 3.0 * inverse.powi(2) * t
            + second.y * 3.0 * inverse * t.powi(2)
            + end.y * t.powi(3),
    )
}

fn unit_normal(first: DocumentPoint, second: DocumentPoint) -> DocumentPoint {
    let tangent = unit_tangent(first, second);
    DocumentPoint::new(-tangent.y, tangent.x)
}

fn unit_tangent(first: DocumentPoint, second: DocumentPoint) -> DocumentPoint {
    let x = second.x - first.x;
    let y = second.y - first.y;
    let length = (x * x + y * y).sqrt();
    if length <= f32::EPSILON {
        DocumentPoint::new(1.0, 0.0)
    } else {
        DocumentPoint::new(x / length, y / length)
    }
}

fn distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    ((second.x - first.x).powi(2) + (second.y - first.y).powi(2)).sqrt()
}
