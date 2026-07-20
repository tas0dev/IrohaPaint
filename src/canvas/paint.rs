use std::fmt::Write;

use viewkit::draw_command::{DrawCommand, SvgCommand};
use viewkit::prelude::{Point, Rect, SvgData};
use viewkit::view::PaintContext;

use crate::document::{BezierNode, BezierPath, Document, DocumentPoint, DocumentRect, ObjectKind};
use crate::editor::EditorTool;

use super::coordinates::CanvasTransform;
use super::interaction::{Interaction, ResizeHandle, ShapeDraftKind, kind_bounds};

const HANDLE_SIZE: f32 = 8.0;
const CONTROL_HANDLE_SIZE: f32 = 6.0;

pub fn paint_editor_canvas(
    document: &Document,
    transform: CanvasTransform,
    interaction: &Interaction,
    active_tool: EditorTool,
    selected_node: Option<(crate::document::ObjectId, usize)>,
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

    for layer in document.layers() {
        for object in layer.objects() {
            if let Some(preview) = interaction.preview_kind(object.id()) {
                paint_kind(&preview, transform, bounds, context);
            } else {
                paint_kind(object.kind(), transform, bounds, context);
            }
        }
    }

    paint_draft(interaction, transform, bounds, context);

    if let Some(selected_id) = document.selected_object()
        && let Some(object) = document.object(selected_id)
    {
        let selection_kind = interaction
            .preview_kind(selected_id)
            .unwrap_or_else(|| object.kind().clone());
        if matches!(active_tool, EditorTool::NodeEdit | EditorTool::Pen)
            && let ObjectKind::Path { path } = &selection_kind
        {
            let selected_index = selected_node
                .filter(|(object_id, _)| *object_id == selected_id)
                .map(|(_, index)| index);
            paint_path_nodes(path, selected_index, transform, bounds, context);
        } else {
            paint_selection(kind_bounds(&selection_kind), transform, bounds, context);
        }
    }

    context.display_list.push(DrawCommand::PopClip);
}

fn paint_kind(
    kind: &ObjectKind,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    match kind {
        ObjectKind::Rectangle { bounds } => {
            let rect = transform.document_rect_to_canvas(*bounds, canvas_bounds);
            context.display_list.push(DrawCommand::FillRect {
                rect,
                color: context.theme.colors.elevated_surface,
            });
            context.display_list.push(DrawCommand::StrokeRect {
                rect,
                color: context.theme.colors.border_strong,
                width: 1.0,
            });
        }
        ObjectKind::Ellipse { bounds } => {
            let rect = transform.document_rect_to_canvas(*bounds, canvas_bounds);
            context.display_list.push(DrawCommand::FillEllipse {
                rect,
                color: context.theme.colors.elevated_surface,
            });
            context.display_list.push(DrawCommand::StrokeEllipse {
                rect,
                color: context.theme.colors.border_strong,
                width: 1.0,
            });
        }
        ObjectKind::Path { path } => {
            paint_svg_path(path, transform, canvas_bounds, context);
        }
    }
}

fn paint_svg_path(
    path: &BezierPath,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    if path.nodes().len() < 2 || canvas_bounds.is_empty() {
        return;
    }

    let nodes = path.nodes();
    let mut commands = String::new();
    write_move(&mut commands, nodes[0].position, transform, canvas_bounds);
    for segment in nodes.windows(2) {
        write_curve(
            &mut commands,
            segment[0].handle_out,
            segment[1].handle_in,
            segment[1].position,
            transform,
            canvas_bounds,
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
        );
        commands.push('Z');
    }

    paint_svg_commands(
        &commands,
        2.0,
        context.theme.colors.text_primary,
        canvas_bounds,
        context,
    );
}

fn paint_svg_commands(
    commands: &str,
    stroke_width: f32,
    tint: viewkit::prelude::Color,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><path d="{commands}" fill="none" stroke="#000" stroke-width="{stroke_width}" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        width = canvas_bounds.size.width.max(1.0),
        height = canvas_bounds.size.height.max(1.0),
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
        } => {
            let bounds = DocumentRect::from_points(*start, *current);
            let kind = match kind {
                ShapeDraftKind::Rectangle => ObjectKind::Rectangle { bounds },
                ShapeDraftKind::Ellipse => ObjectKind::Ellipse { bounds },
            };
            paint_kind(&kind, transform, canvas_bounds, context);
            paint_selection(bounds, transform, canvas_bounds, context);
        }
        Interaction::PlacingPathNode {
            path_id: None,
            position,
            handle_out,
            ..
        } => {
            let node = BezierNode::smooth(*position, *handle_out);
            paint_node_controls(&node, true, transform, canvas_bounds, context);
        }
        _ => {}
    }
}

fn paint_path_nodes(
    path: &BezierPath,
    selected_node: Option<usize>,
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    if let Some(index) = selected_node
        && let Some(node) = path.nodes().get(index)
    {
        paint_node_controls(node, true, transform, canvas_bounds, context);
    }

    for (index, node) in path.nodes().iter().enumerate() {
        let center = transform.document_to_canvas(node.position, canvas_bounds);
        let size = HANDLE_SIZE;
        let rect = Rect::new(center.x - size / 2.0, center.y - size / 2.0, size, size);
        context.display_list.push(DrawCommand::FillRect {
            rect,
            color: if selected_node == Some(index) {
                context.theme.colors.accent
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
    for handle in &visible_handles {
        write_move(&mut commands, node.position, transform, canvas_bounds);
        write_line(&mut commands, *handle, transform, canvas_bounds);
    }
    paint_svg_commands(
        &commands,
        1.0,
        context.theme.colors.accent,
        canvas_bounds,
        context,
    );

    for handle in visible_handles {
        let center = transform.document_to_canvas(handle, canvas_bounds);
        let rect = Rect::new(
            center.x - CONTROL_HANDLE_SIZE / 2.0,
            center.y - CONTROL_HANDLE_SIZE / 2.0,
            CONTROL_HANDLE_SIZE,
            CONTROL_HANDLE_SIZE,
        );
        context.display_list.push(DrawCommand::FillEllipse {
            rect,
            color: if selected {
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

fn point_distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    ((first.x - second.x).powi(2) + (first.y - second.y).powi(2)).sqrt()
}

fn write_move(
    commands: &mut String,
    point: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
) {
    let point = local_canvas_point(point, transform, canvas_bounds);
    let _ = write!(commands, "M{:.3},{:.3} ", point.x, point.y);
}

fn write_line(
    commands: &mut String,
    point: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
) {
    let point = local_canvas_point(point, transform, canvas_bounds);
    let _ = write!(commands, "L{:.3},{:.3} ", point.x, point.y);
}

fn write_curve(
    commands: &mut String,
    control_1: DocumentPoint,
    control_2: DocumentPoint,
    end: DocumentPoint,
    transform: CanvasTransform,
    canvas_bounds: Rect,
) {
    let control_1 = local_canvas_point(control_1, transform, canvas_bounds);
    let control_2 = local_canvas_point(control_2, transform, canvas_bounds);
    let end = local_canvas_point(end, transform, canvas_bounds);
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
) -> Point {
    let point = transform.document_to_canvas(point, canvas_bounds);
    Point::new(
        point.x - canvas_bounds.origin.x,
        point.y - canvas_bounds.origin.y,
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
