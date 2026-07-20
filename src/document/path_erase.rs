use super::{BezierNode, BezierPath, DocumentPoint, NodeKind};

const MIN_SAMPLES_PER_CURVE: usize = 64;
const MAX_SAMPLES_PER_CURVE: usize = 2048;
const BOUNDARY_STEPS: usize = 10;
const END_EPSILON: f32 = 0.0001;

#[derive(Clone, Copy)]
struct Cubic {
    p0: DocumentPoint,
    p1: DocumentPoint,
    p2: DocumentPoint,
    p3: DocumentPoint,
    width0: f32,
    width1: f32,
}

pub(super) fn erase_path(
    path: &BezierPath,
    centers: &[DocumentPoint],
    radius: f32,
) -> Option<Vec<BezierPath>> {
    if path.nodes().len() < 2 || centers.is_empty() || radius <= 0.0 {
        return None;
    }

    let was_closed = path.is_closed();
    let mut changed = false;
    let mut runs = Vec::<Vec<Cubic>>::new();
    let mut current = Vec::<Cubic>::new();
    let mut previous_reaches_end = false;

    let nodes = path.nodes();
    let segment_count = if was_closed {
        nodes.len()
    } else {
        nodes.len() - 1
    };
    let mut first_starts_at_zero = false;
    for segment_index in 0..segment_count {
        let first = nodes[segment_index];
        let second = nodes[(segment_index + 1) % nodes.len()];
        let curve = Cubic {
            p0: first.position,
            p1: first.handle_out,
            p2: second.handle_in,
            p3: second.position,
            width0: first.width,
            width1: second.width,
        };
        let intervals = kept_intervals(curve, centers, radius, &mut changed);
        if segment_index == 0 {
            first_starts_at_zero = intervals
                .first()
                .is_some_and(|(start, _)| *start <= END_EPSILON);
        }

        for (interval_index, (start, end)) in intervals.iter().copied().enumerate() {
            let connects = interval_index == 0
                && segment_index > 0
                && previous_reaches_end
                && start <= END_EPSILON;
            if !connects && !current.is_empty() {
                runs.push(std::mem::take(&mut current));
            }
            current.push(cubic_subsegment(curve, start, end));
        }
        previous_reaches_end = intervals
            .last()
            .is_some_and(|(_, end)| *end >= 1.0 - END_EPSILON);
        if !previous_reaches_end && !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        runs.push(current);
    }
    if !changed {
        return None;
    }

    if was_closed && previous_reaches_end && first_starts_at_zero && runs.len() > 1 {
        let first = runs.remove(0);
        if let Some(last) = runs.last_mut() {
            last.extend(first);
        }
    }

    Some(runs.into_iter().filter_map(path_from_run).collect())
}

fn kept_intervals(
    curve: Cubic,
    centers: &[DocumentPoint],
    radius: f32,
    changed: &mut bool,
) -> Vec<(f32, f32)> {
    let mut intervals = Vec::new();
    let mut previous_t = 0.0;
    let mut previous_erased = point_is_erased(cubic_point(curve, 0.0), centers, radius);
    *changed |= previous_erased;
    let mut kept_start = (!previous_erased).then_some(0.0);
    let sample_count = ((control_polygon_length(curve) / (radius * 0.5).max(0.01)).ceil() as usize)
        .clamp(MIN_SAMPLES_PER_CURVE, MAX_SAMPLES_PER_CURVE);

    for sample in 1..=sample_count {
        let t = sample as f32 / sample_count as f32;
        let erased = point_is_erased(cubic_point(curve, t), centers, radius);
        *changed |= erased;
        if erased != previous_erased {
            let boundary = transition(curve, centers, radius, previous_t, t, previous_erased);
            if erased {
                if let Some(start) = kept_start.take()
                    && boundary - start > END_EPSILON
                {
                    intervals.push((start, boundary));
                }
            } else {
                kept_start = Some(boundary);
            }
        }
        previous_t = t;
        previous_erased = erased;
    }

    if let Some(start) = kept_start
        && 1.0 - start > END_EPSILON
    {
        intervals.push((start, 1.0));
    }
    intervals
}

fn control_polygon_length(curve: Cubic) -> f32 {
    point_distance(curve.p0, curve.p1)
        + point_distance(curve.p1, curve.p2)
        + point_distance(curve.p2, curve.p3)
}

fn point_distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    let x = second.x - first.x;
    let y = second.y - first.y;
    (x * x + y * y).sqrt()
}

fn transition(
    curve: Cubic,
    centers: &[DocumentPoint],
    radius: f32,
    mut low: f32,
    mut high: f32,
    low_erased: bool,
) -> f32 {
    for _ in 0..BOUNDARY_STEPS {
        let middle = (low + high) * 0.5;
        if point_is_erased(cubic_point(curve, middle), centers, radius) == low_erased {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) * 0.5
}

fn point_is_erased(point: DocumentPoint, centers: &[DocumentPoint], radius: f32) -> bool {
    let radius_squared = radius * radius;
    centers.iter().any(|center| {
        let x = point.x - center.x;
        let y = point.y - center.y;
        x * x + y * y <= radius_squared
    })
}

fn cubic_point(curve: Cubic, t: f32) -> DocumentPoint {
    let one_minus_t = 1.0 - t;
    let a = one_minus_t * one_minus_t * one_minus_t;
    let b = 3.0 * one_minus_t * one_minus_t * t;
    let c = 3.0 * one_minus_t * t * t;
    let d = t * t * t;
    DocumentPoint::new(
        curve.p0.x * a + curve.p1.x * b + curve.p2.x * c + curve.p3.x * d,
        curve.p0.y * a + curve.p1.y * b + curve.p2.y * c + curve.p3.y * d,
    )
}

fn cubic_subsegment(curve: Cubic, start: f32, end: f32) -> Cubic {
    let (left, _) = split_cubic(curve, end);
    if start <= 0.0 {
        return left;
    }
    let relative_start = (start / end.max(f32::EPSILON)).clamp(0.0, 1.0);
    split_cubic(left, relative_start).1
}

fn split_cubic(curve: Cubic, t: f32) -> (Cubic, Cubic) {
    let p01 = lerp_point(curve.p0, curve.p1, t);
    let p12 = lerp_point(curve.p1, curve.p2, t);
    let p23 = lerp_point(curve.p2, curve.p3, t);
    let p012 = lerp_point(p01, p12, t);
    let p123 = lerp_point(p12, p23, t);
    let middle = lerp_point(p012, p123, t);
    let width = lerp(curve.width0, curve.width1, t);
    (
        Cubic {
            p0: curve.p0,
            p1: p01,
            p2: p012,
            p3: middle,
            width0: curve.width0,
            width1: width,
        },
        Cubic {
            p0: middle,
            p1: p123,
            p2: p23,
            p3: curve.p3,
            width0: width,
            width1: curve.width1,
        },
    )
}

fn path_from_run(curves: Vec<Cubic>) -> Option<BezierPath> {
    let first = curves.first()?;
    let mut nodes = vec![BezierNode {
        position: first.p0,
        handle_in: first.p0,
        handle_out: first.p1,
        kind: NodeKind::Corner,
        width: first.width0,
    }];
    for curve in curves {
        if let Some(previous) = nodes.last_mut() {
            previous.handle_out = curve.p1;
        }
        nodes.push(BezierNode {
            position: curve.p3,
            handle_in: curve.p2,
            handle_out: curve.p3,
            kind: NodeKind::Corner,
            width: curve.width1,
        });
    }
    BezierPath::from_nodes(nodes)
}

fn lerp_point(first: DocumentPoint, second: DocumentPoint, t: f32) -> DocumentPoint {
    DocumentPoint::new(lerp(first.x, second.x, t), lerp(first.y, second.y, t))
}

fn lerp(first: f32, second: f32, t: f32) -> f32 {
    first + (second - first) * t
}
