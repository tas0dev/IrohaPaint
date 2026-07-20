use std::fmt::Write;

use viewkit::draw_command::{DrawCommand, SvgCommand};
use viewkit::prelude::{Point, Rect, SvgData};
use viewkit::view::PaintContext;

use crate::document::{Document, DocumentPoint, DocumentRect, ObjectKind};

use super::coordinates::CanvasTransform;
use super::interaction::{Interaction, ResizeHandle, ShapeDraftKind, kind_bounds};

const HANDLE_SIZE: f32 = 8.0;

pub fn paint_editor_canvas(
    document: &Document,
    transform: CanvasTransform,
    interaction: &Interaction,
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
        paint_selection(kind_bounds(&selection_kind), transform, bounds, context);
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
        ObjectKind::Path { points } => {
            paint_svg_path(points, transform, canvas_bounds, context);
        }
    }
}

fn paint_svg_path(
    points: &[DocumentPoint],
    transform: CanvasTransform,
    canvas_bounds: Rect,
    context: &mut PaintContext<'_>,
) {
    if points.len() < 2 || canvas_bounds.is_empty() {
        return;
    }

    let mut path = String::new();
    for (index, point) in points.iter().enumerate() {
        let point = transform.document_to_canvas(*point, canvas_bounds);
        let local_x = point.x - canvas_bounds.origin.x;
        let local_y = point.y - canvas_bounds.origin.y;
        let command = if index == 0 { 'M' } else { 'L' };
        let _ = write!(path, "{command}{local_x:.3},{local_y:.3} ");
    }

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><path d="{path}" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
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
            tint: Some(context.theme.colors.text_primary),
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
        Interaction::DrawingPath { points } => {
            paint_svg_path(points, transform, canvas_bounds, context);
        }
        _ => {}
    }
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
