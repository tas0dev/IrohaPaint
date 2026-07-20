use std::fmt::Write;

use crate::document::{
    BezierPath, CanvasSize, Document, DocumentColor, DocumentPoint, DocumentRect, ObjectId,
    ObjectKind, PAINT_TILE_SIZE, PaintLayer, StrokeCap, StrokeJoin, StrokeStyle,
    variable_stroke_outlines,
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
    let mut mask_id = 0_u64;
    for (layer_index, layer) in document.layers().iter().enumerate() {
        if !layer.is_visible() {
            continue;
        }
        let clip_id = if let Some(base_index) = document.clip_base_layer(layer_index) {
            let id = mask_id;
            mask_id += 1;
            let _ = write!(
                source,
                r#"<defs><mask id="layer-clip-{id}" maskUnits="userSpaceOnUse" x="{x:.3}" y="{y:.3}" width="{width:.3}" height="{height:.3}" style="mask-type:luminance">"#,
                x = bounds.x,
                y = bounds.y,
                width = bounds.width,
                height = bounds.height,
            );
            let base = &document.layers()[base_index];
            if base.is_visible() {
                write_layer_mask_content(&mut source, base)?;
            }
            source.push_str("</mask></defs>");
            Some(id)
        } else {
            None
        };
        if let Some(id) = clip_id {
            let _ = write!(source, r#"<g mask="url(#layer-clip-{id})">"#);
        } else {
            source.push_str("<g>");
        }
        write_layer_content(&mut source, layer, &mut mask_id)?;
        source.push_str("</g>");
    }
    source.push_str("</svg>\n");
    Ok(ExportedSvg {
        source,
        width: bounds.width,
        height: bounds.height,
    })
}

pub(crate) fn serialize_layer(
    document: &Document,
    layer_index: usize,
    viewport: DocumentRect,
    preview: Option<(ObjectId, &ObjectKind)>,
    extra: Option<&ObjectKind>,
) -> Result<String, ExportError> {
    let Some(layer) = document.layers().get(layer_index) else {
        return Err(ExportError::EmptyDocument);
    };
    let mut source = String::new();
    let _ = write!(
        source,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.3}" height="{height:.3}" viewBox="{x:.3} {y:.3} {width:.3} {height:.3}">"#,
        x = viewport.x,
        y = viewport.y,
        width = viewport.width.max(0.1),
        height = viewport.height.max(0.1),
    );
    let mut mask_id = 1_u64;
    source.push_str("<g>");
    write_paint_layer(&mut source, layer.paint())?;
    for object in layer.objects() {
        if preview.is_some_and(|(id, _)| id == object.id()) {
            continue;
        }
        write_object(&mut source, object.kind(), &mut mask_id);
    }
    if let Some((_, kind)) = preview {
        write_object(&mut source, kind, &mut mask_id);
    }
    if let Some(extra) = extra {
        write_object(&mut source, extra, &mut mask_id);
    }
    source.push_str("</g></svg>");
    Ok(source)
}

fn write_layer_content(
    source: &mut String,
    layer: &crate::document::Layer,
    mask_id: &mut u64,
) -> Result<(), ExportError> {
    write_paint_layer(source, layer.paint())?;
    for object in layer.objects() {
        write_object(source, object.kind(), mask_id);
    }
    Ok(())
}

fn write_layer_mask_content(
    source: &mut String,
    layer: &crate::document::Layer,
) -> Result<(), ExportError> {
    write_paint_mask(source, layer.paint())?;
    for object in layer.objects() {
        match object.kind() {
            ObjectKind::Rectangle { bounds, style } => {
                let _ = write!(
                    source,
                    r#"<rect x="{x:.3}" y="{y:.3}" width="{width:.3}" height="{height:.3}" fill="white" fill-opacity="{fill_opacity:.3}" stroke="white" stroke-opacity="{stroke_opacity:.3}" stroke-width="{stroke_width:.3}"/>"#,
                    x = bounds.x,
                    y = bounds.y,
                    width = bounds.width,
                    height = bounds.height,
                    fill_opacity = style.fill.alpha as f32 / 255.0,
                    stroke_opacity = style.stroke.color.alpha as f32 / 255.0,
                    stroke_width = style.stroke.width.max(0.0),
                );
            }
            ObjectKind::Ellipse { bounds, style } => {
                let _ = write!(
                    source,
                    r#"<ellipse cx="{cx:.3}" cy="{cy:.3}" rx="{rx:.3}" ry="{ry:.3}" fill="white" fill-opacity="{fill_opacity:.3}" stroke="white" stroke-opacity="{stroke_opacity:.3}" stroke-width="{stroke_width:.3}"/>"#,
                    cx = bounds.x + bounds.width * 0.5,
                    cy = bounds.y + bounds.height * 0.5,
                    rx = bounds.width * 0.5,
                    ry = bounds.height * 0.5,
                    fill_opacity = style.fill.alpha as f32 / 255.0,
                    stroke_opacity = style.stroke.color.alpha as f32 / 255.0,
                    stroke_width = style.stroke.width.max(0.0),
                );
            }
            ObjectKind::Path {
                path,
                style,
                variable_width,
                cutouts,
            } => {
                let mut commands = String::new();
                if *variable_width {
                    if path.is_closed() && style.fill.alpha > 0 {
                        write_path_commands(&mut commands, path);
                        let _ = write!(
                            source,
                            r#"<path d="{commands}" fill="white" fill-opacity="{:.3}" stroke="none"/>"#,
                            style.fill.alpha as f32 / 255.0,
                        );
                        commands.clear();
                    }
                    for outline in variable_stroke_outlines(path, style.stroke) {
                        write_path_commands(&mut commands, &outline);
                    }
                    let _ = write!(
                        source,
                        r#"<path d="{commands}" fill="white" fill-opacity="{:.3}" fill-rule="evenodd" stroke="none"/>"#,
                        style.stroke.color.alpha as f32 / 255.0,
                    );
                } else {
                    write_path_commands(&mut commands, path);
                    let _ = write!(
                        source,
                        r#"<path d="{commands}" fill="white" fill-opacity="{fill_opacity:.3}" stroke="white" stroke-opacity="{stroke_opacity:.3}" stroke-width="{width:.3}" stroke-linecap="{cap}" stroke-linejoin="{join}"/>"#,
                        fill_opacity = if path.is_closed() {
                            style.fill.alpha as f32 / 255.0
                        } else {
                            0.0
                        },
                        stroke_opacity = style.stroke.color.alpha as f32 / 255.0,
                        width = style.stroke.width.max(0.0),
                        cap = cap_name(style.stroke.cap),
                        join = join_name(style.stroke.join),
                    );
                }
                if !cutouts.is_empty() {
                    let mut cutout_commands = String::new();
                    for cutout in cutouts {
                        write_path_commands(&mut cutout_commands, cutout);
                    }
                    let _ = write!(
                        source,
                        r#"<path d="{cutout_commands}" fill="black" stroke="black"/>"#,
                    );
                }
            }
        }
    }
    Ok(())
}

fn write_paint_mask(source: &mut String, layer: &PaintLayer) -> Result<(), ExportError> {
    for tile in layer.tiles() {
        let mut pixels = tile.pixels().to_vec();
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = pixel[3];
            pixel[0] = alpha;
            pixel[1] = alpha;
            pixel[2] = alpha;
        }
        let size = tiny_skia::IntSize::from_wh(PAINT_TILE_SIZE, PAINT_TILE_SIZE)
            .ok_or(ExportError::InvalidDimensions)?;
        let pixmap =
            tiny_skia::Pixmap::from_vec(pixels, size).ok_or(ExportError::InvalidDimensions)?;
        let png = pixmap
            .encode_png()
            .map_err(|error| ExportError::Png(error.to_string()))?;
        let bounds = tile.document_bounds();
        let _ = write!(
            source,
            r#"<image x="{x:.3}" y="{y:.3}" width="{width:.3}" height="{height:.3}" href="data:image/png;base64,{data}"/>"#,
            x = bounds.x,
            y = bounds.y,
            width = bounds.width,
            height = bounds.height,
            data = base64(&png),
        );
    }
    Ok(())
}

fn write_paint_layer(source: &mut String, layer: &PaintLayer) -> Result<(), ExportError> {
    for tile in layer.tiles() {
        let mut pixels = tile.pixels().to_vec();
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u16::from(pixel[3]);
            pixel[0] = ((u16::from(pixel[0]) * alpha + 127) / 255) as u8;
            pixel[1] = ((u16::from(pixel[1]) * alpha + 127) / 255) as u8;
            pixel[2] = ((u16::from(pixel[2]) * alpha + 127) / 255) as u8;
        }
        let size = tiny_skia::IntSize::from_wh(PAINT_TILE_SIZE, PAINT_TILE_SIZE)
            .ok_or(ExportError::InvalidDimensions)?;
        let pixmap =
            tiny_skia::Pixmap::from_vec(pixels, size).ok_or(ExportError::InvalidDimensions)?;
        let png = pixmap
            .encode_png()
            .map_err(|error| ExportError::Png(error.to_string()))?;
        let bounds = tile.document_bounds();
        let _ = write!(
            source,
            r#"<image x="{x:.3}" y="{y:.3}" width="{width:.3}" height="{height:.3}" href="data:image/png;base64,{data}"/>"#,
            x = bounds.x,
            y = bounds.y,
            width = bounds.width,
            height = bounds.height,
            data = base64(&png),
        );
    }
    Ok(())
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or_default();
        let third = chunk.get(2).copied().unwrap_or_default();
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn write_object(source: &mut String, kind: &ObjectKind, mask_id: &mut u64) {
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
        ObjectKind::Path {
            path,
            style,
            variable_width,
            cutouts,
        } => {
            if path.nodes().len() < 2 {
                return;
            }
            let active_mask = (!cutouts.is_empty()).then(|| {
                let id = *mask_id;
                *mask_id += 1;
                write_cutout_mask(source, id, cutouts);
                let _ = write!(source, r#"<g mask="url(#cutout-{id})">"#);
                id
            });
            if *variable_width {
                if path.is_closed() && style.fill.alpha > 0 {
                    write_path_fill(source, path, style.fill);
                }
                write_variable_stroke(source, path, style.stroke);
                if active_mask.is_some() {
                    source.push_str("</g>");
                }
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
            if active_mask.is_some() {
                source.push_str("</g>");
            }
        }
    }
}

fn write_cutout_mask(source: &mut String, id: u64, cutouts: &[BezierPath]) {
    let mut commands = String::new();
    for cutout in cutouts {
        write_path_commands(&mut commands, cutout);
    }
    let _ = write!(
        source,
        r#"<defs><mask id="cutout-{id}"><rect x="-10%" y="-10%" width="120%" height="120%" fill="white"/><path d="{commands}" fill="black" stroke="black"/></mask></defs>"#,
    );
}

fn write_path_fill(source: &mut String, path: &BezierPath, fill: DocumentColor) {
    let mut commands = String::new();
    write_path_commands(&mut commands, path);
    let _ = write!(
        source,
        r##"<path d="{commands}" fill="{color}" fill-opacity="{opacity:.3}" stroke="none"/>"##,
        color = color_hex(fill),
        opacity = fill.alpha as f32 / 255.0,
    );
}

fn write_variable_stroke(source: &mut String, path: &BezierPath, stroke: StrokeStyle) {
    let outlines = variable_stroke_outlines(path, stroke);
    if outlines.is_empty() {
        return;
    }
    let mut commands = String::new();
    for outline in &outlines {
        write_path_commands(&mut commands, outline);
    }
    let _ = write!(
        source,
        r##"<path d="{commands}" fill="{color}" fill-opacity="{opacity:.3}" fill-rule="evenodd" stroke="none"/>"##,
        color = color_hex(stroke.color),
        opacity = stroke.color.alpha as f32 / 255.0,
    );
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
    let mut bounds = None;
    for (index, layer) in document.layers().iter().enumerate() {
        if !layer.is_visible() {
            continue;
        }
        let mut current = layer_bounds(layer);
        if let Some(base_index) = document.clip_base_layer(index) {
            let base = &document.layers()[base_index];
            current = if base.is_visible() {
                current.and_then(|current| {
                    layer_bounds(base).and_then(|base| intersection(current, base))
                })
            } else {
                None
            };
        }
        if let Some(current) = current {
            bounds = Some(bounds.map_or(current, |bounds| union(bounds, current)));
        }
    }
    bounds
}

fn layer_bounds(layer: &crate::document::Layer) -> Option<DocumentRect> {
    layer
        .objects()
        .iter()
        .filter_map(|object| object_bounds(object.kind()))
        .chain(layer.paint().bounds())
        .reduce(union)
}

fn object_bounds(kind: &ObjectKind) -> Option<DocumentRect> {
    match kind {
        ObjectKind::Rectangle { bounds, style } | ObjectKind::Ellipse { bounds, style } => {
            Some(expand(*bounds, style.stroke.width.max(0.0) / 2.0))
        }
        ObjectKind::Path {
            path,
            style,
            variable_width,
            ..
        } => {
            if *variable_width {
                variable_stroke_outlines(path, style.stroke)
                    .iter()
                    .filter_map(path_curve_bounds)
                    .reduce(union)
            } else {
                path_curve_bounds(path)
                    .map(|bounds| expand(bounds, style.stroke.width.max(0.0) / 2.0))
            }
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

fn intersection(first: DocumentRect, second: DocumentRect) -> Option<DocumentRect> {
    let x = first.x.max(second.x);
    let y = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    (right > x && bottom > y).then_some(DocumentRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
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
