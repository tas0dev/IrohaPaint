use viewkit::prelude::*;

use super::icon_button;
use crate::brush::{BrushKind, BrushLibrary};
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
    pub eraser_mode: State<usize>,
    pub smoothing: State<f32>,
    pub inspected_object: State<Option<ObjectId>>,
    pub layer_name: State<String>,
    pub layer_name_settings: ModalState,
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
    let filling = tool == EditorTool::Fill;
    let erasing = tool == EditorTool::Eraser;
    let brush_kind = if matches!(
        tool,
        EditorTool::BlobBrush | EditorTool::Fill | EditorTool::Paint
    ) {
        BrushKind::Paint
    } else {
        BrushKind::Line
    };
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
            let brush = brushes.get().active(brush_kind).clone();
            bindings.stroke_color.set(view_color(brush.color));
            bindings.brush_width.set(brush.width);
            bindings.smoothing.set(brush.smoothing);
        }
    }
    if selected_object.is_none() {
        let brush = brushes.get().active(brush_kind).clone();
        let color = view_color(brush.color);
        if bindings.stroke_color.get() != color {
            bindings.stroke_color.set(color);
        }
        if brush_kind == BrushKind::Line && bindings.brush_width.get() != brush.width {
            bindings.brush_width.set(brush.width);
        }
        if bindings.smoothing.get() != brush.smoothing {
            bindings.smoothing.set(brush.smoothing);
        }
    }
    let selected_layer = current_document.selected_layer();
    let selected_layer_clipped = selected_layer
        .and_then(|index| current_document.layers().get(index))
        .is_some_and(|layer| layer.is_clipped());
    let layer_rows = current_document
        .layers()
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, layer)| {
            layer.folder().is_none_or(|id| {
                current_document
                    .folders()
                    .iter()
                    .find(|folder| folder.id() == id)
                    .is_none_or(|folder| folder.is_expanded())
            })
        })
        .map(|(index, layer)| {
            let layer_name = layer.name().to_owned();
            let visible = layer.is_visible();
            let folder_name = layer.folder().and_then(|id| {
                current_document
                    .folders()
                    .iter()
                    .find(|folder| folder.id() == id)
                    .map(|folder| folder.name().to_owned())
            });
            let mut list_row = ListRow::new(layer_name.clone());
            if let Some(folder_name) = folder_name {
                list_row = list_row.subtitle(folder_name);
            }
            let mut row = HStack::new()
                .alignment(StackAlignment::Center)
                .gap(StackGap::ExtraSmall);
            if let Some(preview) = layer_preview(&current_document, index) {
                row = row.child(preview.frame(32.0, 32.0));
            }
            row.child(
                list_row
                    .selected(selected_layer == Some(index))
                    .on_select({
                        let document = document.clone();
                        let canvas = canvas.clone();
                        move || {
                            document.update(|document| document.select_layer(index));
                            clear_canvas_selection(&canvas);
                        }
                    })
                    .layout()
                    .flex_grow(1.0),
            )
            .child(
                icon_button::view(if visible { "eye" } else { "eye-off" }).on_click({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        if document.update(|document| document.toggle_layer_visibility(index)) {
                            clear_canvas_selection(&canvas);
                        }
                    }
                }),
            )
            .child(icon_button::view("pencil").on_click({
                let document = document.clone();
                let canvas = canvas.clone();
                let name = bindings.layer_name.clone();
                let modal = bindings.layer_name_settings.clone();
                move || {
                    document.update(|document| document.select_layer(index));
                    clear_canvas_selection(&canvas);
                    name.set(layer_name.clone());
                    modal.open();
                }
            }))
        })
        .collect::<Vec<_>>();
    let folder_rows = current_document
        .folders()
        .iter()
        .map(|folder| {
            let id = folder.id();
            let visible = folder.is_visible();
            let expanded = folder.is_expanded();
            HStack::new()
                .alignment(StackAlignment::Center)
                .gap(StackGap::ExtraSmall)
                .child(
                    ListRow::new(folder.name())
                        .subtitle(if expanded { "Expanded" } else { "Collapsed" })
                        .on_select({
                            let document = document.clone();
                            move || {
                                document.update(|document| document.toggle_folder_expanded(id));
                            }
                        })
                        .layout()
                        .flex_grow(1.0),
                )
                .child(
                    icon_button::view(if visible { "eye" } else { "eye-off" }).on_click({
                        let document = document.clone();
                        let canvas = canvas.clone();
                        move || {
                            if document.update(|document| document.toggle_folder_visibility(id)) {
                                clear_canvas_selection(&canvas);
                            }
                        }
                    }),
                )
        })
        .collect::<Vec<_>>();
    let editing_fill = !painting_blob
        && !painting_raster
        && !filling
        && !erasing
        && bindings.color_target.get() == 1;
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
        .children(folder_rows)
        .children(layer_rows)
        .child(
            HStack::new()
                .gap(StackGap::ExtraSmall)
                .child(icon_button::view("plus").on_click({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        document.update(Document::add_layer);
                        clear_canvas_selection(&canvas);
                    }
                }))
                .child(icon_button::view("layers").on_click({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        if document.update(Document::create_folder_from_selected) {
                            clear_canvas_selection(&canvas);
                        }
                    }
                }))
                .child(
                    icon_button::view("trash-2")
                        .enabled(current_document.layers().len() > 1)
                        .on_click({
                            let document = document.clone();
                            let canvas = canvas.clone();
                            move || {
                                if document.update(Document::delete_selected_layer) {
                                    let mut canvas = canvas.get_mut();
                                    canvas.active_pen_path = None;
                                    canvas.selected_nodes.clear();
                                    canvas.hovered_node = None;
                                    canvas.hovered_segment = None;
                                }
                            }
                        }),
                )
                .child(
                    icon_button::view("arrow-up")
                        .enabled(
                            selected_layer
                                .is_some_and(|index| index + 1 < current_document.layers().len()),
                        )
                        .on_click({
                            let document = document.clone();
                            let canvas = canvas.clone();
                            move || {
                                if document.update(Document::move_selected_layer_up) {
                                    clear_canvas_selection(&canvas);
                                }
                            }
                        }),
                )
                .child(
                    icon_button::view("arrow-down")
                        .enabled(selected_layer.is_some_and(|index| index > 0))
                        .on_click({
                            let document = document.clone();
                            let canvas = canvas.clone();
                            move || {
                                if document.update(Document::move_selected_layer_down) {
                                    clear_canvas_selection(&canvas);
                                }
                            }
                        }),
                )
                .child(
                    icon_button::view("link-2")
                        .style(if selected_layer_clipped {
                            ButtonStyle::Standard
                        } else {
                            ButtonStyle::Ghost
                        })
                        .enabled(selected_layer.is_some_and(|index| index > 0))
                        .on_click({
                            let document = document.clone();
                            let canvas = canvas.clone();
                            move || {
                                if document.update(Document::toggle_selected_layer_clipping) {
                                    clear_canvas_selection(&canvas);
                                }
                            }
                        }),
                ),
        )
        .child(
            Text::new(if erasing {
                "Eraser"
            } else if painting_blob || painting_raster || filling {
                "Paint"
            } else {
                "Appearance"
            })
            .weight(700),
        )
        .child(Divider::new());

    if !painting_blob && !painting_raster && !filling && !erasing {
        content = content.child(
            SegmentedControl::new(bindings.color_target.binding())
                .item(0, "Stroke")
                .item(1, "Fill"),
        );
    }
    if !erasing {
        content = content.child(Text::new(color_label(selected_color)));
    }

    content = if erasing {
        let object_mode = bindings.eraser_mode.get() == 1;
        content
            .child(
                SegmentedControl::new(bindings.eraser_mode.binding())
                    .item(0, "Partial")
                    .item(1, "Object"),
            )
            .child(Text::new(if object_mode {
                "Drag across an object to remove it."
            } else {
                "Drag across a vector stroke to cut away that section."
            }))
    } else if editing_fill {
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
            .child(icon_button::view("circle-off").on_click({
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
        let content = content.child(
            ColorPicker::new(bindings.stroke_color.binding()).on_commit({
                let brushes = brushes.clone();
                let document = document.clone();
                let active_tool = active_tool.clone();
                move |color| {
                    let color = document_color(color);
                    brushes.update(|library| {
                        library.update_active(brush_kind, |brush| brush.color = color)
                    });
                    if edits_selection(active_tool.get()) {
                        document.update(|document| document.set_selected_stroke_color(color));
                    }
                }
            }),
        );
        if filling {
            content
        } else {
            content
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
                        .on_commit({
                            let brushes = brushes.clone();
                            move |width| {
                                brushes.update(|library| {
                                    library.update_active(BrushKind::Paint, |brush| {
                                        brush.paint_width = width
                                    })
                                });
                            }
                        })
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
                                    library
                                        .update_active(BrushKind::Line, |brush| brush.width = width)
                                });
                                if edits_selection(active_tool.get()) {
                                    document.update(|document| {
                                        document.set_selected_stroke_width(width)
                                    });
                                }
                            }
                        })
                })
        }
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
                                library
                                    .update_active(brush_kind, |brush| brush.smoothing = smoothing)
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

fn clear_canvas_selection(canvas: &CanvasController) {
    let mut canvas = canvas.get_mut();
    canvas.active_pen_path = None;
    canvas.selected_nodes.clear();
    canvas.hovered_node = None;
    canvas.hovered_segment = None;
}

fn layer_preview(document: &Document, layer_index: usize) -> Option<Image> {
    let crate::document::CanvasSize::Custom { width, height } = document.properties().canvas_size
    else {
        return None;
    };
    let source = crate::export::serialize_layer_content_for_canvas(
        document,
        layer_index,
        crate::document::DocumentRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
        None,
        None,
    )
    .ok()?;
    let svg = SvgData::decode(source.as_bytes()).ok()?;
    let image = ImageData::from_svg(&svg, 64, 64).ok()?;
    Some(Image::new(image))
}
