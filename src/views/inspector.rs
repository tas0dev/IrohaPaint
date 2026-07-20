use viewkit::prelude::*;

use crate::brush::BrushLibrary;
use crate::canvas::CanvasController;
use crate::document::{Document, DocumentColor, NodeKind, ObjectId};
use crate::editor::EditorTool;

pub struct InspectorBindings {
    pub stroke_color: State<Color>,
    pub fill_color: State<Color>,
    pub color_target: State<usize>,
    pub brush_width: State<f32>,
    pub blob_width: State<f32>,
    pub paint_size: State<f32>,
    pub paint_opacity: State<f32>,
    pub paint_softness: State<f32>,
    pub smoothing: State<f32>,
    pub inspected_object: State<Option<ObjectId>>,
}

pub fn view(
    document: State<Document>,
    canvas: CanvasController,
    brushes: State<BrushLibrary>,
    active_tool: State<EditorTool>,
    bindings: InspectorBindings,
) -> impl View + 'static {
    let current_document = document.get();
    let tool = active_tool.get();
    let painting_blob = tool == EditorTool::BlobBrush;
    let painting_raster = tool == EditorTool::Paint;
    let selected_object = if edits_selection(tool) {
        current_document.selected_object()
    } else {
        None
    };
    if bindings.inspected_object.get() != selected_object {
        bindings.inspected_object.set(selected_object);
        if let Some(style) = selected_object
            .and_then(|id| current_document.object(id))
            .map(|object| object.style())
        {
            bindings.stroke_color.set(view_color(style.stroke.color));
            bindings.fill_color.set(view_color(style.fill));
            bindings.brush_width.set(style.stroke.width);
        } else {
            let brush = brushes.get().active().clone();
            bindings.stroke_color.set(view_color(brush.color));
            bindings.brush_width.set(brush.width);
            bindings.smoothing.set(brush.smoothing);
        }
    }
    let selected_layer = current_document.selected_layer();
    let layer_rows = current_document
        .layers()
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            ListRow::new(layer.name())
                .selected(selected_layer == Some(index))
                .on_select({
                    let document = document.clone();
                    move || document.update(|document| document.select_layer(index))
                })
        })
        .collect::<Vec<_>>();
    let editing_fill = !painting_blob && !painting_raster && bindings.color_target.get() == 1;
    let selected_color = if editing_fill {
        bindings.fill_color.get()
    } else {
        bindings.stroke_color.get()
    };

    let mut content = VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::Medium)
        .child(Text::new("Layers").weight(700))
        .child(Divider::new())
        .children(layer_rows)
        .child(
            Text::new(if painting_blob || painting_raster {
                "Paint"
            } else {
                "Appearance"
            })
            .weight(700),
        )
        .child(Divider::new());

    if !painting_blob && !painting_raster {
        content = content.child(
            SegmentedControl::new(bindings.color_target.binding())
                .item(0, "Stroke")
                .item(1, "Fill"),
        );
    }
    content = content.child(Text::new(color_label(selected_color)));

    content = if editing_fill {
        content
            .child(ColorPicker::new(bindings.fill_color.binding()).on_commit({
                let document = document.clone();
                let active_tool = active_tool.clone();
                move |color| {
                    if edits_selection(active_tool.get()) {
                        document.update(|document| {
                            document.set_selected_fill_color(document_color(color))
                        });
                    }
                }
            }))
            .child(Button::new("No Fill").on_click({
                let fill_color = bindings.fill_color.clone();
                let document = document.clone();
                let active_tool = active_tool.clone();
                move || {
                    fill_color.set(Color::TRANSPARENT);
                    if edits_selection(active_tool.get()) {
                        document.update(|document| {
                            document.set_selected_fill_color(DocumentColor::TRANSPARENT)
                        });
                    }
                }
            }))
    } else {
        content
            .child(
                ColorPicker::new(bindings.stroke_color.binding()).on_commit({
                    let brushes = brushes.clone();
                    let document = document.clone();
                    let active_tool = active_tool.clone();
                    move |color| {
                        let color = document_color(color);
                        brushes
                            .update(|library| library.update_active(|brush| brush.color = color));
                        if edits_selection(active_tool.get()) {
                            document.update(|document| document.set_selected_stroke_color(color));
                        }
                    }
                }),
            )
            .child(Text::new(format!(
                "Size — {:.1} px",
                if painting_blob {
                    bindings.blob_width.get()
                } else if painting_raster {
                    bindings.paint_size.get()
                } else {
                    bindings.brush_width.get()
                }
            )))
            .child(if painting_blob {
                Slider::new(bindings.blob_width.binding())
                    .range(1.0..=200.0)
                    .step(1.0)
            } else if painting_raster {
                Slider::new(bindings.paint_size.binding())
                    .range(1.0..=256.0)
                    .step(1.0)
            } else {
                Slider::new(bindings.brush_width.binding())
                    .range(0.5..=32.0)
                    .step(0.5)
                    .on_commit({
                        let brushes = brushes.clone();
                        let document = document.clone();
                        let active_tool = active_tool.clone();
                        move |width| {
                            brushes.update(|library| {
                                library.update_active(|brush| brush.width = width)
                            });
                            if edits_selection(active_tool.get()) {
                                document
                                    .update(|document| document.set_selected_stroke_width(width));
                            }
                        }
                    })
            })
    };

    if painting_raster {
        content = content
            .child(Text::new(format!(
                "Opacity — {:.0}%",
                bindings.paint_opacity.get() * 100.0
            )))
            .child(
                Slider::new(bindings.paint_opacity.binding())
                    .range(0.01..=1.0)
                    .step(0.01),
            )
            .child(Text::new(format!(
                "Softness — {:.0}%",
                bindings.paint_softness.get() * 100.0
            )))
            .child(
                Slider::new(bindings.paint_softness.binding())
                    .range(0.0..=1.0)
                    .step(0.05),
            );
    }

    if matches!(tool, EditorTool::Pencil | EditorTool::BlobBrush) {
        content = content
            .child(Text::new(format!(
                "Stabilizer — {:.0}%",
                bindings.smoothing.get() * 100.0
            )))
            .child(
                Slider::new(bindings.smoothing.binding())
                    .range(0.0..=1.0)
                    .step(0.05)
                    .on_commit({
                        let brushes = brushes.clone();
                        move |smoothing| {
                            brushes.update(|library| {
                                library.update_active(|brush| brush.smoothing = smoothing)
                            });
                        }
                    }),
            );
    }

    if tool == EditorTool::NodeEdit {
        content = content
            .child(Text::new("Nodes").weight(700))
            .child(Divider::new())
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(node_kind_button(
                        "Corner",
                        NodeKind::Corner,
                        document.clone(),
                        canvas.clone(),
                    ))
                    .child(node_kind_button(
                        "Smooth",
                        NodeKind::Smooth,
                        document.clone(),
                        canvas.clone(),
                    ))
                    .child(node_kind_button(
                        "Symmetric",
                        NodeKind::Symmetric,
                        document.clone(),
                        canvas.clone(),
                    )),
            )
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(Button::new("Smooth Curve").on_click({
                        let document = document.clone();
                        let canvas = canvas.clone();
                        move || {
                            if let Some((id, indices)) = selected_path_nodes(&canvas) {
                                document
                                    .update(|document| document.smooth_path_nodes(id, &indices));
                            }
                        }
                    }))
                    .child(Button::new("Simplify").on_click({
                        let document = document.clone();
                        let canvas = canvas.clone();
                        move || {
                            if let Some((id, indices)) = selected_path_nodes(&canvas) {
                                let tolerance = 1.5 / canvas.get().transform.zoom();
                                let changed = document.update(|document| {
                                    document.simplify_path_nodes(id, &indices, tolerance)
                                });
                                if changed {
                                    canvas.get_mut().selected_nodes.clear();
                                }
                            }
                        }
                    })),
            );
    }

    Padding::all(12.0).content(content)
}

fn color_label(color: Color) -> String {
    if color.alpha == 0 {
        String::from("None")
    } else {
        format!("#{:02X}{:02X}{:02X}", color.red, color.green, color.blue)
    }
}

fn document_color(color: Color) -> DocumentColor {
    DocumentColor::rgba(color.red, color.green, color.blue, color.alpha)
}

fn view_color(color: DocumentColor) -> Color {
    Color::rgba(color.red, color.green, color.blue, color.alpha)
}

fn edits_selection(tool: EditorTool) -> bool {
    matches!(tool, EditorTool::Select | EditorTool::NodeEdit)
}

fn node_kind_button(
    title: &'static str,
    kind: NodeKind,
    document: State<Document>,
    canvas: CanvasController,
) -> Button {
    Button::new(title).on_click(move || {
        if let Some((id, indices)) = selected_path_nodes(&canvas) {
            document.update(|document| document.set_path_node_kinds(id, &indices, kind));
        }
    })
}

fn selected_path_nodes(
    canvas: &CanvasController,
) -> Option<(crate::document::ObjectId, Vec<usize>)> {
    let state = canvas.get();
    let id = state.selected_nodes.first()?.0;
    let indices = state
        .selected_nodes
        .iter()
        .filter_map(|(object_id, index)| (*object_id == id).then_some(*index))
        .collect::<Vec<_>>();
    (!indices.is_empty()).then_some((id, indices))
}
