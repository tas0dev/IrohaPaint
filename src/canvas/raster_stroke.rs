use crate::document::{DocumentPoint, PaintDab};

pub fn interpolate_dabs(
    from: DocumentPoint,
    to: DocumentPoint,
    distance_since_dab: &mut f32,
    spacing: f32,
    template: PaintDab,
) -> Vec<PaintDab> {
    let delta_x = to.x - from.x;
    let delta_y = to.y - from.y;
    let length = (delta_x * delta_x + delta_y * delta_y).sqrt();
    if length <= f32::EPSILON {
        return Vec::new();
    }

    let spacing = spacing.max(0.1);
    let mut offset = (spacing - *distance_since_dab).max(0.0);
    let mut dabs = Vec::new();
    while offset <= length {
        let amount = offset / length;
        dabs.push(PaintDab {
            center: DocumentPoint::new(from.x + delta_x * amount, from.y + delta_y * amount),
            ..template
        });
        offset += spacing;
    }
    *distance_since_dab = (*distance_since_dab + length) % spacing;
    dabs
}
