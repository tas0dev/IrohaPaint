use std::fmt::Write;

use viewkit::draw_command::{DrawCommand, ImageCommand, ImageSampling, SvgCommand};
use viewkit::prelude::{ImageData, Point, Rect, SvgData};
use viewkit::view::PaintContext;

use crate::document::{
    BezierNode, BezierPath, CanvasSize, Document, DocumentColor, DocumentPoint, DocumentRect,
    NodeComponent, ObjectId, ObjectKind, ObjectStyle, PaintLayer, StrokeCap, StrokeJoin,
    StrokeStyle, variable_stroke_outlines,
};
use crate::editor::EditorTool;

use super::coordinates::CanvasTransform;
use super::hit_test::SegmentHit;
use super::interaction::{Interaction, ResizeHandle, ShapeDraftKind, kind_bounds};

const HANDLE_SIZE: f32 = 8.0;
const CONTROL_HANDLE_SIZE: f32 = 6.0;

pub(crate) struct NodePresentation<'a> {
    pub selected: &'a [(ObjectId, usize)],
    pub hovered: Option<(ObjectId, usize, NodeComponent)>,
    pub segment: Option<(ObjectId, SegmentHit)>,
}

pub fn paint_editor_canvas(
    document: &Document,
    transform: CanvasTransform,
    interaction: &Interaction,
    active_tool: EditorTool,
    nodes: NodePresentation<'_>,
    bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    context.display_list.push(DrawCommand::FillRect {
        rect: bounds,
        color: context.theme.colors.surface_subtle,
    });
    context
        .display_list
        .push(DrawCommand::PushClip { rect: bounds });

    let artboard = if let CanvasSize::Custom { width, height } = document.properties().canvas_size {
        let artboard = transform.document_rect_to_canvas(
            DocumentRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            bounds,
        );
        let background = document.properties().background;
        context.display_list.push(DrawCommand::FillRect {
            rect: artboard,
            color: if background.alpha == 0 {
                context.theme.colors.elevated_surface
            } else {
                view_color(background)
            },
        });
        Some(artboard)
    } else {
        None
    };

    if let Some(artboard) = artboard {
        context
            .display_list
            .push(DrawCommand::PushClip { rect: artboard });
    }

    for layer in document.layers() {
        if !layer.is_visible() {
            continue;
        }
        paint_raster_layer(layer.paint(), transform, bounds, context);
        for object in layer.objects() {
            if let Some(preview) = interaction.preview_kind(object.id()) {
                paint_kind(&preview, transform, bounds, context);
            } else {
                paint_kind(object.kind(), transform, bounds, context);
            }
        }
    }

    paint_draft(interaction, transform, bounds, context);

    if artboard.is_some() {
        context.display_list.push(DrawCommand::PopClip);
    }

    if let Some(artboard) = artboard {
        context.display_list.push(DrawCommand::StrokeRect {
            rect: artboard,
            color: context.theme.colors.border,
            width: 1.0,
        });
    }

    if let Some(selected_id) = document.selected_object()
        && let Some(object) = document.object(selected_id)
    {
        let selection_kind = interaction
            .preview_kind(selected_id)
            .unwrap_or_else(|| object.kind().clone());
        if matches!(active_tool, EditorTool::NodeEdit | EditorTool::Pen)
            && let ObjectKind::Path { path, .. } = &selection_kind
        {
            let selected_indices = nodes
                .selected
                .iter()
                .filter_map(|(object_id, index)| (*object_id == selected_id).then_some(*index))
                .collect::<Vec<_>>();
            let hovered = (active_tool == EditorTool::NodeEdit)
                .then_some(nodes.hovered)
                .flatten()
                .and_then(|(object_id, index, component)| {
                    (object_id == selected_id).then_some((index, component))
                });
            paint_path_nodes(path, &selected_indices, hovered, transform, bounds, context);
            if active_tool == EditorTool::NodeEdit
                && let Some((object_id, hit)) = nodes.segment
                && object_id == selected_id
            {
                paint_segment_insertion(hit.point, transform, bounds, context);
            }
        } else if !matches!(
            active_tool,
            EditorTool::Pencil
                | EditorTool::Paint
                | EditorTool::Fill
                | EditorTool::Eraser
                | EditorTool::BlobBrush
        ) {
            paint_selection(kind_bounds(&selection_kind), transform, bounds, context);
        }
    }

    context.display_list.push(DrawCommand::PopClip);
}

fn paint_raster_layer(
    layer: &PaintLayer,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    for tile in layer.tiles() {
        let Ok(image) = ImageData::from_rgba8(
            crate::document::PAINT_TILE_SIZE,
            crate::document::PAINT_TILE_SIZE,
            tile.pixels().to_vec(),
        ) else {
            continue;
        };
        context.display_list.push(DrawCommand::DrawImage {
            command: ImageCommand {
                image,
                bounds: transform.document_rect_to_canvas(tile.document_bounds(), canvas_bounds),
                opacity: 1.0,
                sampling: ImageSampling::Bilinear,
            },
        });
    }
}

fn paint_kind(
    kind: &ObjectKind,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    match kind {
        ObjectKind::Rectangle { bounds, style } => {
            let rect = transform.document_rect_to_canvas(*bounds, canvas_bounds);
            if style.fill.alpha > 0 {
                context.display_list.push(DrawCommand::FillRect {
                    rect,
                    color: view_color(style.fill),
                });
            }
            context.display_list.push(DrawCommand::StrokeRect {
                rect,
                color: view_color(style.stroke.color),
                width: style.stroke.width * transform.zoom(),
            });
        }
        ObjectKind::Ellipse { bounds, style } => {
            let rect = transform.document_rect_to_canvas(*bounds, canvas_bounds);
            if style.fill.alpha > 0 {
                context.display_list.push(DrawCommand::FillEllipse {
                    rect,
                    color: view_color(style.fill),
                });
            }
            context.display_list.push(DrawCommand::StrokeEllipse {
                rect,
                color: view_color(style.stroke.color),
                width: style.stroke.width * transform.zoom(),
            });
        }
        ObjectKind::Path {
            path,
            style,
            variable_width,
            cutouts,
        } => {
            if *variable_width {
                if path.is_closed() && style.fill.alpha > 0 {
                    paint_svg_path(
                        path,
                        ObjectStyle {
                            stroke: StrokeStyle {
                                color: DocumentColor::TRANSPARENT,
                                width: 0.1,
                                ..StrokeStyle::default()
                            },
                            fill: style.fill,
                        },
                        transform,
                        canvas_bounds,
                        context,
                        cutouts,
                    );
                }
                paint_variable_stroke(path, style.stroke, transform, canvas_bounds, context);
            } else {
                paint_svg_path(path, *style, transform, canvas_bounds, context, cutouts);
            }
        }
    }
}

fn paint_variable_stroke(
    path: &BezierPath,
    stroke: StrokeStyle,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    let outlines = variable_stroke_outlines(path, stroke);
    if outlines.is_empty() || canvas_bounds.is_empty() {
        return;
    }
    let svg_bounds = path_canvas_bounds(
        path,
        transform,
        canvas_bounds,
        stroke.width * transform.zoom() / 2.0 + 3.0,
    );
    let mut commands = String::new();
    for outline in &outlines {
        write_path_commands(&mut commands, outline, transform, canvas_bounds, svg_bounds);
    }
    let color = format!(
        "#{:02X}{:02X}{:02X}",
        stroke.color.red, stroke.color.green, stroke.color.blue
    );
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><path d="{commands}" fill="{color}" fill-opacity="{opacity}" fill-rule="evenodd" stroke="none"/></svg>"##,
        width = svg_bounds.size.width.max(1.0),
        height = svg_bounds.size.height.max(1.0),
        opacity = stroke.color.alpha as f32 / 255.0,
    );
    let Ok(svg) = SvgData::decode(svg.as_bytes()) else {
        return;
    };
    context.display_list.push(DrawCommand::DrawSvg {
        command: SvgCommand {
            svg,
            bounds: svg_bounds,
            opacity: 1.0,
            tint: None,
        },
    });
}

fn write_path_commands(
    commands: &mut String,
    path: &BezierPath,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    svg_bounds: Rect,
) {
    let Some(first) = path.nodes().first() else {
        return;
    };
    write_move(
        commands,
        first.position,
        transform,
        canvas_bounds,
        svg_bounds,
    );
    for segment in path.nodes().windows(2) {
        write_curve(
            commands,
            segment[0].handle_out,
            segment[1].handle_in,
            segment[1].position,
            transform,
            canvas_bounds,
            svg_bounds,
        );
    }
    if path.is_closed() {
        let last = path.nodes().last().expect("a closed path has nodes");
        write_curve(
            commands,
            last.handle_out,
            first.handle_in,
            first.position,
            transform,
            canvas_bounds,
            svg_bounds,
        );
        commands.push('Z');
    }
}

fn paint_svg_path(
    path: &BezierPath,
    style: ObjectStyle,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
    cutouts: &[BezierPath],
) {
    if path.nodes().len() < 2 || canvas_bounds.is_empty() {
        return;
    }

    let nodes = path.nodes();
    let svg_bounds = path_canvas_bounds(
        path,
        transform,
        canvas_bounds,
        style.stroke.width * transform.zoom() / 2.0 + 2.0,
    );
    let mut commands = String::new();
    write_move(
        &mut commands,
        nodes[0].position,
        transform,
        canvas_bounds,
        svg_bounds,
    );
    for segment in nodes.windows(2) {
        write_curve(
            &mut commands,
            segment[0].handle_out,
            segment[1].handle_in,
            segment[1].position,
            transform,
            canvas_bounds,
            svg_bounds,
        );
    }
    if path.is_closed() {
        let first = nodes.first().expect("a closed path has nodes");
        let last = nodes.last().expect("a closed path has nodes");
        write_curve(
            &mut commands,
            last.handle_out,
            first.handle_in,
            first.position,
            transform,
            canvas_bounds,
            svg_bounds,
        );
        commands.push('Z');
    }

    let fill = if path.is_closed() && style.fill.alpha > 0 {
        format!(
            "#{:02X}{:02X}{:02X}",
            style.fill.red, style.fill.green, style.fill.blue
        )
    } else {
        String::from("none")
    };
    let stroke_color = format!(
        "#{:02X}{:02X}{:02X}",
        style.stroke.color.red, style.stroke.color.green, style.stroke.color.blue
    );
    let mut mask = String::new();
    let mask_attribute = if cutouts.is_empty() {
        ""
    } else {
        let mut cutout_commands = String::new();
        for cutout in cutouts {
            write_path_commands(
                &mut cutout_commands,
                cutout,
                transform,
                canvas_bounds,
                svg_bounds,
            );
        }
        mask = format!(
            r#"<defs><mask id="cutouts" maskUnits="userSpaceOnUse"><rect width="100%" height="100%" fill="white"/><path d="{cutout_commands}" fill="black" stroke="black"/></mask></defs>"#,
        );
        r#" mask="url(#cutouts)""#
    };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">{mask}<path d="{commands}" fill="{fill}" fill-opacity="{fill_opacity}" stroke="{stroke}" stroke-opacity="{stroke_opacity}" stroke-width="{stroke_width}" stroke-linecap="{stroke_cap}" stroke-linejoin="{stroke_join}"{mask_attribute}/></svg>"##,
        width = svg_bounds.size.width.max(1.0),
        height = svg_bounds.size.height.max(1.0),
        fill_opacity = style.fill.alpha as f32 / 255.0,
        stroke = stroke_color,
        stroke_opacity = style.stroke.color.alpha as f32 / 255.0,
        stroke_width = (style.stroke.width * transform.zoom()).max(0.1),
        stroke_cap = stroke_cap_name(style.stroke.cap),
        stroke_join = stroke_join_name(style.stroke.join),
    );
    let Ok(svg) = SvgData::decode(svg.as_bytes()) else {
        return;
    };
    context.display_list.push(DrawCommand::DrawSvg {
        command: SvgCommand {
            svg,
            bounds: svg_bounds,
            opacity: 1.0,
            tint: None,
        },
    });
}

fn view_color(color: DocumentColor) -> viewkit::prelude::Color {
    viewkit::prelude::Color::rgba(color.red, color.green, color.blue, color.alpha)
}

fn paint_svg_commands(
    commands: &str,
    stroke: StrokeStyle,
    tint: viewkit::prelude::Color,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><path d="{commands}" fill="none" stroke="#000" stroke-width="{stroke_width}" stroke-linecap="{stroke_cap}" stroke-linejoin="{stroke_join}"/></svg>"##,
        width = canvas_bounds.size.width.max(1.0),
        height = canvas_bounds.size.height.max(1.0),
        stroke_width = stroke.width.max(0.1),
        stroke_cap = stroke_cap_name(stroke.cap),
        stroke_join = stroke_join_name(stroke.join),
    );
    let Ok(svg) = SvgData::decode(svg.as_bytes()) else {
        return;
    };

    context.display_list.push(DrawCommand::DrawSvg {
        command: SvgCommand {
            svg,
            bounds: canvas_bounds,
            opacity: 1.0,
            tint: Some(tint),
        },
    });
}

fn paint_draft(
    interaction: &Interaction,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    match interaction {
        Interaction::DrawingShape {
            kind,
            start,
            current,
            style,
        } => {
            let bounds = DocumentRect::from_points(*start, *current);
            let kind = match kind {
                ShapeDraftKind::Rectangle => ObjectKind::Rectangle {
                    bounds,
                    style: *style,
                },
                ShapeDraftKind::Ellipse => ObjectKind::Ellipse {
                    bounds,
                    style: *style,
                },
            };
            paint_kind(&kind, transform, canvas_bounds, context);
            paint_selection(bounds, transform, canvas_bounds, context);
        }
        Interaction::DrawingPencil {
            preview: Some(path),
            brush,
            ..
        } => {
            paint_variable_stroke(
                path,
                brush.stroke_style(),
                transform,
                canvas_bounds,
                context,
            );
        }
        Interaction::DrawingBlob {
            preview: Some(path),
            style,
            ..
        } => {
            paint_svg_path(path, *style, transform, canvas_bounds, context, &[]);
        }
        Interaction::PlacingPathNode {
            path_id: None,
            position,
            handle_out,
            ..
        } => {
            let node = BezierNode::smooth(*position, *handle_out);
            paint_node_controls(&node, true, None, transform, canvas_bounds, context);
        }
        Interaction::SelectingNodes { start, current, .. } => {
            let rect = transform.document_rect_to_canvas(
                DocumentRect::from_points(*start, *current),
                canvas_bounds,
            );
            context.display_list.push(DrawCommand::StrokeRect {
                rect,
                color: context.theme.colors.accent,
                width: 1.0,
            });
        }
        _ => {}
    }
}

fn paint_path_nodes(
    path: &BezierPath,
    selected_nodes: &[usize],
    hovered_node: Option<(usize, NodeComponent)>,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    for &index in selected_nodes {
        if let Some(node) = path.nodes().get(index) {
            let hovered = hovered_node
                .filter(|(hovered_index, _)| *hovered_index == index)
                .map(|(_, component)| component);
            paint_node_controls(node, true, hovered, transform, canvas_bounds, context);
        }
    }

    for (index, node) in path.nodes().iter().enumerate() {
        let center = transform.document_to_canvas(node.position, canvas_bounds);
        let size = HANDLE_SIZE;
        let rect = Rect::new(center.x - size / 2.0, center.y - size / 2.0, size, size);
        context.display_list.push(DrawCommand::FillRect {
            rect,
            color: if selected_nodes.contains(&index) {
                context.theme.colors.accent
            } else if hovered_node == Some((index, NodeComponent::Anchor)) {
                context.theme.colors.accent_hovered
            } else {
                context.theme.colors.elevated_surface
            },
        });
        context.display_list.push(DrawCommand::StrokeRect {
            rect,
            color: context.theme.colors.accent,
            width: 1.0,
        });
    }
}

fn paint_node_controls(
    node: &BezierNode,
    selected: bool,
    hovered: Option<NodeComponent>,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    let visible_handles = [node.handle_in, node.handle_out]
        .into_iter()
        .filter(|handle| point_distance(*handle, node.position) > f32::EPSILON)
        .collect::<Vec<_>>();
    if visible_handles.is_empty() {
        return;
    }

    let mut commands = String::new();
    let control_bounds = points_canvas_bounds(
        &[node.position, node.handle_in, node.handle_out],
        transform,
        canvas_bounds,
        2.0,
    );
    for handle in &visible_handles {
        write_move(
            &mut commands,
            node.position,
            transform,
            canvas_bounds,
            control_bounds,
        );
        write_line(
            &mut commands,
            *handle,
            transform,
            canvas_bounds,
            control_bounds,
        );
    }
    paint_svg_commands(
        &commands,
        StrokeStyle {
            width: 1.0,
            ..StrokeStyle::default()
        },
        context.theme.colors.accent,
        control_bounds,
        context,
    );

    for (component, handle) in [
        (NodeComponent::HandleIn, node.handle_in),
        (NodeComponent::HandleOut, node.handle_out),
    ] {
        if !visible_handles.contains(&handle) {
            continue;
        }
        let center = transform.document_to_canvas(handle, canvas_bounds);
        let rect = Rect::new(
            center.x - CONTROL_HANDLE_SIZE / 2.0,
            center.y - CONTROL_HANDLE_SIZE / 2.0,
            CONTROL_HANDLE_SIZE,
            CONTROL_HANDLE_SIZE,
        );
        context.display_list.push(DrawCommand::FillEllipse {
            rect,
            color: if hovered == Some(component) {
                context.theme.colors.accent_hovered
            } else if selected {
                context.theme.colors.elevated_surface
            } else {
                context.theme.colors.surface
            },
        });
        context.display_list.push(DrawCommand::StrokeEllipse {
            rect,
            color: context.theme.colors.accent,
            width: 1.0,
        });
    }
}

fn stroke_cap_name(cap: StrokeCap) -> &'static str {
    match cap {
        StrokeCap::Butt => "butt",
        StrokeCap::Round => "round",
        StrokeCap::Square => "square",
    }
}

fn stroke_join_name(join: StrokeJoin) -> &'static str {
    match join {
        StrokeJoin::Miter => "miter",
        StrokeJoin::Round => "round",
        StrokeJoin::Bevel => "bevel",
    }
}

fn point_distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    ((first.x - second.x).powi(2) + (first.y - second.y).powi(2)).sqrt()
}

fn write_move(
    commands: &mut String,
    point: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    drawing_bounds: Rect,
) {
    let point = local_canvas_point(point, transform, canvas_bounds, drawing_bounds);
    let _ = write!(commands, "M{:.3},{:.3} ", point.x, point.y);
}

fn write_line(
    commands: &mut String,
    point: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    drawing_bounds: Rect,
) {
    let point = local_canvas_point(point, transform, canvas_bounds, drawing_bounds);
    let _ = write!(commands, "L{:.3},{:.3} ", point.x, point.y);
}

fn write_curve(
    commands: &mut String,
    control_1: DocumentPoint,
    control_2: DocumentPoint,
    end: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    drawing_bounds: Rect,
) {
    let control_1 = local_canvas_point(control_1, transform, canvas_bounds, drawing_bounds);
    let control_2 = local_canvas_point(control_2, transform, canvas_bounds, drawing_bounds);
    let end = local_canvas_point(end, transform, canvas_bounds, drawing_bounds);
    let _ = write!(
        commands,
        "C{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} ",
        control_1.x, control_1.y, control_2.x, control_2.y, end.x, end.y
    );
}

fn local_canvas_point(
    point: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    drawing_bounds: Rect,
) -> Point {
    let point = transform.document_to_canvas(point, canvas_bounds);
    Point::new(
        point.x - drawing_bounds.origin.x,
        point.y - drawing_bounds.origin.y,
    )
}

fn path_canvas_bounds(
    path: &BezierPath,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    padding: f32,
) -> Rect {
    let points = path
        .nodes()
        .iter()
        .flat_map(|node| [node.position, node.handle_in, node.handle_out]);
    document_points_canvas_bounds(points, transform, canvas_bounds, padding)
}

fn points_canvas_bounds(
    points: &[DocumentPoint],
    transform: CanvasTransform,
    canvas_bounds: Rect,
    padding: f32,
) -> Rect {
    document_points_canvas_bounds(points.iter().copied(), transform, canvas_bounds, padding)
}

fn document_points_canvas_bounds(
    points: impl IntoIterator<Item = DocumentPoint>,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    padding: f32,
) -> Rect {
    let mut points = points.into_iter();
    let first = transform.document_to_canvas(points.next().unwrap_or_default(), canvas_bounds);
    let (mut min_x, mut max_x) = (first.x, first.x);
    let (mut min_y, mut max_y) = (first.y, first.y);
    for point in points {
        let point = transform.document_to_canvas(point, canvas_bounds);
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Rect::new(
        min_x - padding,
        min_y - padding,
        (max_x - min_x + padding * 2.0).max(1.0),
        (max_y - min_y + padding * 2.0).max(1.0),
    )
}

fn paint_selection(
    bounds: DocumentRect,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    let rect = transform.document_rect_to_canvas(bounds, canvas_bounds);
    context.display_list.push(DrawCommand::StrokeRect {
        rect,
        color: context.theme.colors.accent,
        width: 1.0,
    });

    for handle in ResizeHandle::ALL {
        let center = transform.document_to_canvas(handle.position(bounds), canvas_bounds);
        paint_handle(center, context);
    }
}

fn paint_handle(center: Point, context: &mut PaintContext<'_>) {
    let rect = Rect::new(
        center.x - HANDLE_SIZE / 2.0,
        center.y - HANDLE_SIZE / 2.0,
        HANDLE_SIZE,
        HANDLE_SIZE,
    );
    context.display_list.push(DrawCommand::FillRect {
        rect,
        color: context.theme.colors.elevated_surface,
    });
    context.display_list.push(DrawCommand::StrokeRect {
        rect,
        color: context.theme.colors.accent,
        width: 1.0,
    });
}

fn paint_segment_insertion(
    point: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    let center = transform.document_to_canvas(point, canvas_bounds);
    let rect = Rect::new(
        center.x - CONTROL_HANDLE_SIZE / 2.0,
        center.y - CONTROL_HANDLE_SIZE / 2.0,
        CONTROL_HANDLE_SIZE,
        CONTROL_HANDLE_SIZE,
    );
    context.display_list.push(DrawCommand::FillEllipse {
        rect,
        color: context.theme.colors.elevated_surface,
    });
    context.display_list.push(DrawCommand::StrokeEllipse {
        rect,
        color: context.theme.colors.accent_hovered,
        width: 1.0,
    });
}
