use std::fmt::Write;

use crate::document::{
    BezierPath, CanvasSize, Document, DocumentColor, DocumentPoint, DocumentRect, ObjectKind,
    StrokeCap, StrokeJoin,
};

use super::ExportError;

pub struct ExportedSvg {
    pub source: String,
    pub width: f32,
    pub height: f32,
}

pub fn serialize(document: &Document) -> Result<ExportedSvg, ExportError> {
    let bounds = match document.properties().canvas_size {
        CanvasSize::FitArtwork => artwork_bounds(document).ok_or(ExportError::EmptyDocument)?,
        CanvasSize::Custom { width, height } => DocumentRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
    };
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return Err(ExportError::InvalidDimensions);
    }

    let mut source = String::new();
    let _ = write!(
        source,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.3}" height="{height:.3}" viewBox="{x:.3} {y:.3} {width:.3} {height:.3}">"#,
        x = bounds.x,
        y = bounds.y,
        width = bounds.width,
        height = bounds.height,
    );
    for layer in document.layers() {
        source.push_str("<g>");
        for object in layer.objects() {
            write_object(&mut source, object.kind());
        }
        source.push_str("</g>");
    }
    source.push_str("</svg>\n");
    Ok(ExportedSvg {
        source,
        width: bounds.width,
        height: bounds.height,
    })
}

fn write_object(source: &mut String, kind: &ObjectKind) {
    match kind {
        ObjectKind::Rectangle { bounds, style } => {
            let _ = write!(
                source,
                r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="{}" fill-opacity="{:.3}" stroke="{}" stroke-opacity="{:.3}" stroke-width="{:.3}"/>"##,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                fill_name(style.fill),
                style.fill.alpha as f32 / 255.0,
                color_hex(style.stroke.color),
                style.stroke.color.alpha as f32 / 255.0,
                style.stroke.width.max(0.0),
            );
        }
        ObjectKind::Ellipse { bounds, style } => {
            let _ = write!(
                source,
                r##"<ellipse cx="{:.3}" cy="{:.3}" rx="{:.3}" ry="{:.3}" fill="{}" fill-opacity="{:.3}" stroke="{}" stroke-opacity="{:.3}" stroke-width="{:.3}"/>"##,
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
                bounds.width / 2.0,
                bounds.height / 2.0,
                fill_name(style.fill),
                style.fill.alpha as f32 / 255.0,
                color_hex(style.stroke.color),
                style.stroke.color.alpha as f32 / 255.0,
                style.stroke.width.max(0.0),
            );
        }
        ObjectKind::Path { path, style } => {
            if path.nodes().len() < 2 {
                return;
            }
            let mut commands = String::new();
            write_path_commands(&mut commands, path);
            let _ = write!(
                source,
                r##"<path d="{commands}" fill="{fill}" fill-opacity="{fill_opacity:.3}" stroke="{color}" stroke-opacity="{opacity:.3}" stroke-width="{width:.3}" stroke-linecap="{cap}" stroke-linejoin="{join}"/>"##,
                fill = if path.is_closed() {
                    fill_name(style.fill)
                } else {
                    "none".to_owned()
                },
                fill_opacity = style.fill.alpha as f32 / 255.0,
                color = color_hex(style.stroke.color),
                opacity = style.stroke.color.alpha as f32 / 255.0,
                width = style.stroke.width.max(0.0),
                cap = cap_name(style.stroke.cap),
                join = join_name(style.stroke.join),
            );
        }
    }
}

fn color_hex(color: DocumentColor) -> String {
    format!("#{:02X}{:02X}{:02X}", color.red, color.green, color.blue)
}

fn fill_name(color: DocumentColor) -> String {
    if color.alpha == 0 {
        String::from("none")
    } else {
        color_hex(color)
    }
}

fn write_path_commands(commands: &mut String, path: &BezierPath) {
    let Some(first) = path.nodes().first() else {
        return;
    };
    let _ = write!(commands, "M{:.3},{:.3}", first.position.x, first.position.y);
    for nodes in path.nodes().windows(2) {
        write_curve(
            commands,
            nodes[0].handle_out,
            nodes[1].handle_in,
            nodes[1].position,
        );
    }
    if path.is_closed() {
        let last = path.nodes().last().expect("a closed path has nodes");
        write_curve(commands, last.handle_out, first.handle_in, first.position);
        commands.push('Z');
    }
}

fn write_curve(
    commands: &mut String,
    first: DocumentPoint,
    second: DocumentPoint,
    end: DocumentPoint,
) {
    let _ = write!(
        commands,
        "C{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}",
        first.x, first.y, second.x, second.y, end.x, end.y,
    );
}

fn artwork_bounds(document: &Document) -> Option<DocumentRect> {
    document
        .layers()
        .iter()
        .flat_map(|layer| layer.objects())
        .filter_map(|object| object_bounds(object.kind()))
        .reduce(union)
}

fn object_bounds(kind: &ObjectKind) -> Option<DocumentRect> {
    match kind {
        ObjectKind::Rectangle { bounds, style } | ObjectKind::Ellipse { bounds, style } => {
            Some(expand(*bounds, style.stroke.width.max(0.0) / 2.0))
        }
        ObjectKind::Path { path, style } => {
            path_curve_bounds(path).map(|bounds| expand(bounds, style.stroke.width.max(0.0) / 2.0))
        }
    }
}

fn path_curve_bounds(path: &BezierPath) -> Option<DocumentRect> {
    if path.nodes().len() < 2 {
        return None;
    }
    let first = path.nodes().first()?;
    let mut bounds = PointBounds::new(first.position);
    for nodes in path.nodes().windows(2) {
        include_cubic(
            &mut bounds,
            nodes[0].position,
            nodes[0].handle_out,
            nodes[1].handle_in,
            nodes[1].position,
        );
    }
    if path.is_closed() {
        let last = path.nodes().last()?;
        include_cubic(
            &mut bounds,
            last.position,
            last.handle_out,
            first.handle_in,
            first.position,
        );
    }
    Some(bounds.rect())
}

fn include_cubic(
    bounds: &mut PointBounds,
    start: DocumentPoint,
    first: DocumentPoint,
    second: DocumentPoint,
    end: DocumentPoint,
) {
    bounds.include(start);
    bounds.include(end);
    for t in cubic_extrema(start.x, first.x, second.x, end.x)
        .into_iter()
        .chain(cubic_extrema(start.y, first.y, second.y, end.y))
    {
        bounds.include(cubic_point(start, first, second, end, t));
    }
}

fn cubic_extrema(start: f32, first: f32, second: f32, end: f32) -> Vec<f32> {
    let a = -start + 3.0 * first - 3.0 * second + end;
    let b = 2.0 * (start - 2.0 * first + second);
    let c = first - start;
    if a.abs() <= f32::EPSILON {
        if b.abs() <= f32::EPSILON {
            return Vec::new();
        }
        let t = -c / b;
        return (0.0 < t && t < 1.0).then_some(t).into_iter().collect();
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return Vec::new();
    }
    let root = discriminant.sqrt();
    [(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]
        .into_iter()
        .filter(|t| 0.0 < *t && *t < 1.0)
        .collect()
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

fn union(first: DocumentRect, second: DocumentRect) -> DocumentRect {
    let x = first.x.min(second.x);
    let y = first.y.min(second.y);
    let right = (first.x + first.width).max(second.x + second.width);
    let bottom = (first.y + first.height).max(second.y + second.height);
    DocumentRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

fn expand(bounds: DocumentRect, amount: f32) -> DocumentRect {
    DocumentRect {
        x: bounds.x - amount,
        y: bounds.y - amount,
        width: bounds.width + amount * 2.0,
        height: bounds.height + amount * 2.0,
    }
}

struct PointBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl PointBounds {
    fn new(point: DocumentPoint) -> Self {
        Self {
            min_x: point.x,
            min_y: point.y,
            max_x: point.x,
            max_y: point.y,
        }
    }

    fn include(&mut self, point: DocumentPoint) {
        self.min_x = self.min_x.min(point.x);
        self.min_y = self.min_y.min(point.y);
        self.max_x = self.max_x.max(point.x);
        self.max_y = self.max_y.max(point.y);
    }

    fn rect(&self) -> DocumentRect {
        DocumentRect {
            x: self.min_x,
            y: self.min_y,
            width: self.max_x - self.min_x,
            height: self.max_y - self.min_y,
        }
    }
}

fn cap_name(cap: StrokeCap) -> &'static str {
    match cap {
        StrokeCap::Butt => "butt",
        StrokeCap::Round => "round",
        StrokeCap::Square => "square",
    }
}

fn join_name(join: StrokeJoin) -> &'static str {
    match join {
        StrokeJoin::Miter => "miter",
        StrokeJoin::Round => "round",
        StrokeJoin::Bevel => "bevel",
    }
}
