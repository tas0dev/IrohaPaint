use std::path::PathBuf;
use viewkit::prelude::*;

use super::icon_button;
use crate::brush::{BrushKind, BrushLibrary};
use crate::canvas::CanvasController;
use crate::document::{Document, DocumentColor, FolderId, NodeKind, ObjectId};
use crate::editor::EditorTool;

#[derive(Clone)]
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
    pub folder_name: State<String>,
    pub editing_folder: State<Option<FolderId>>,
    pub folder_name_settings: ModalState,
    pub layer_opacity: State<f32>,
    pub inspected_layer: State<Option<usize>>,
    pub project_path: State<Option<PathBuf>>,
    pub layer_scroll: ScrollState,
    pub property_scroll: ScrollState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InspectorPalette {
    ToolProperty,
    Layers,
}

pub fn view(
    document: State<Document>,
    canvas: CanvasController,
    brushes: State<BrushLibrary>,
    active_tool: State<EditorTool>,
    bindings: InspectorBindings,
    palette: InspectorPalette,
) -> Box<dyn View + 'static> {
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
    if bindings.inspected_layer.get() != selected_layer {
        bindings.inspected_layer.set(selected_layer);
        if let Some(layer) = selected_layer.and_then(|index| current_document.layers().get(index)) {
            bindings.layer_opacity.set(layer.opacity());
        }
    }
    let selected_layer_locked = current_document.selected_layer_is_locked();
    let selected_layer_alpha_locked = current_document.selected_layer_is_alpha_locked();
    let selected_layer_clipped = selected_layer
        .and_then(|index| current_document.layers().get(index))
        .is_some_and(|layer| layer.is_clipped());
    let drag_drop = ListDragDropState::new();
    let folder_targets = current_document
        .folders()
        .iter()
        .enumerate()
        .map(|(index, folder)| (index as u64 + 1, folder.id()))
        .collect::<Vec<_>>();
    let layer_folders = current_document
        .layers()
        .iter()
        .map(|layer| layer.folder())
        .collect::<Vec<_>>();
    const LAYER_TARGET_BASE: u64 = 1 << 32;
    const LAYER_ROW_HEIGHT: f32 = 56.0;
    let make_layer_row = |index: usize, layer: &crate::document::Layer| {
        let layer_name = layer.name().to_owned();
        let visible = layer.is_visible();
        let mut list_row = ListRow::new(layer_name.clone());
        let mut details = Vec::new();
        if layer.is_locked() {
            details.push(String::from("Locked"));
        }
        if layer.is_alpha_locked() {
            details.push(String::from("Alpha Lock"));
        }
        if !details.is_empty() {
            list_row = list_row.subtitle(details.join(" · "));
        }
        let mut row = HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::ExtraSmall);
        if layer.folder().is_some() {
            row = row.child(Spacer::new().into_stack_child().width(20.0));
        }
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
                .drop_target(drag_drop.clone(), LAYER_TARGET_BASE + index as u64)
                .on_drop(drag_drop.clone(), {
                    let document = document.clone();
                    let canvas = canvas.clone();
                    let layer_count = current_document.layers().len();
                    let folder_targets = folder_targets.clone();
                    let layer_folders = layer_folders.clone();
                    move |drop_target, delta_y| {
                        let (target, folder) = if let Some(target) = drop_target {
                            if target == 0 {
                                (index, None)
                            } else if let Some((_, folder)) =
                                folder_targets.iter().find(|(id, _)| *id == target)
                            {
                                (index, Some(*folder))
                            } else if target >= LAYER_TARGET_BASE {
                                let target = (target - LAYER_TARGET_BASE) as usize;
                                (
                                    target.min(layer_count.saturating_sub(1)),
                                    layer_folders.get(target).copied().flatten(),
                                )
                            } else {
                                (index, layer_folders[index])
                            }
                        } else {
                            let distance =
                                (delta_y.abs() / LAYER_ROW_HEIGHT).round().max(1.0) as isize;
                            let target = if delta_y > 0.0 {
                                index as isize - distance
                            } else {
                                index as isize + distance
                            }
                            .clamp(0, layer_count.saturating_sub(1) as isize)
                                as usize;
                            (target, layer_folders[index])
                        };
                        if document
                            .update(|document| document.place_layer_at(index, target, folder))
                        {
                            clear_canvas_selection(&canvas);
                        }
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
    };
    let mut layer_rows = current_document
        .layers()
        .iter()
        .enumerate()
        .rev()
        .map(|(index, layer)| (layer.folder(), Some(make_layer_row(index, layer))))
        .collect::<Vec<_>>();
    let mut folder_rows = current_document
        .folders()
        .iter()
        .map(|folder| {
            let id = folder.id();
            let visible = folder.is_visible();
            let expanded = folder.is_expanded();
            let layer_count = current_document
                .layers()
                .iter()
                .filter(|layer| layer.folder() == Some(id))
                .count();
            let mut row = HStack::new()
                .alignment(StackAlignment::Center)
                .gap(StackGap::ExtraSmall);
            if let Some(icon) = crate::icons::icon(if expanded { "folder-open" } else { "folder" })
            {
                row = row.child(Svg::new(icon).frame(20.0, 20.0));
            }
            row = row
                .child(
                    ListRow::new(folder.name())
                        .subtitle(format!(
                            "Folder · {layer_count} {}",
                            if layer_count == 1 { "layer" } else { "layers" }
                        ))
                        .on_select({
                            let document = document.clone();
                            move || {
                                document.update(|document| document.toggle_folder_expanded(id));
                            }
                        })
                        .drop_target(
                            drag_drop.clone(),
                            folder_targets
                                .iter()
                                .find_map(|(target, folder)| (*folder == id).then_some(*target))
                                .expect("each folder has a drag target"),
                        )
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
                .child(icon_button::view("pencil").on_click({
                    let name = bindings.folder_name.clone();
                    let editing = bindings.editing_folder.clone();
                    let modal = bindings.folder_name_settings.clone();
                    let folder_name = folder.name().to_owned();
                    move || {
                        editing.set(Some(id));
                        name.set(folder_name.clone());
                        modal.open();
                    }
                }))
                .child(icon_button::view("trash-2").on_click({
                    let document = document.clone();
                    move || {
                        document.update(|document| document.delete_folder(id));
                    }
                }));
            (id, expanded, Some(row))
        })
        .collect::<Vec<_>>();
    let mut top_level_row = {
        let mut row = HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::ExtraSmall);
        if let Some(icon) = crate::icons::icon("layers") {
            row = row.child(Svg::new(icon).frame(20.0, 20.0));
        }
        Some(
            row.child(
                ListRow::new("Top Level")
                    .subtitle("Drop here to remove from a folder")
                    .drop_target(drag_drop.clone(), 0)
                    .layout()
                    .flex_grow(1.0),
            ),
        )
    };
    let mut hierarchy_rows = Vec::new();
    let mut emitted_folders = Vec::new();
    let mut emitted_top_level = false;
    for position in 0..layer_rows.len() {
        let folder = layer_rows[position].0;
        if let Some(id) = folder {
            if emitted_folders.contains(&id) {
                continue;
            }
            emitted_folders.push(id);
            let expanded = if let Some((_, expanded, row)) = folder_rows
                .iter_mut()
                .find(|(folder_id, _, _)| *folder_id == id)
            {
                if let Some(row) = row.take() {
                    hierarchy_rows.push(row);
                }
                *expanded
            } else {
                true
            };
            if expanded {
                for (row_folder, row) in &mut layer_rows {
                    if *row_folder == Some(id)
                        && let Some(row) = row.take()
                    {
                        hierarchy_rows.push(row);
                    }
                }
            }
        } else if !emitted_top_level {
            emitted_top_level = true;
            if let Some(row) = top_level_row.take() {
                hierarchy_rows.push(row);
            }
            for (row_folder, row) in &mut layer_rows {
                if row_folder.is_none()
                    && let Some(row) = row.take()
                {
                    hierarchy_rows.push(row);
                }
            }
        }
    }
    for (_, _, row) in &mut folder_rows {
        if let Some(row) = row.take() {
            hierarchy_rows.push(row);
        }
    }
    if let Some(row) = top_level_row.take() {
        hierarchy_rows.push(row);
    }
    let layer_content_height = hierarchy_rows.len() as f32 * LAYER_ROW_HEIGHT;
    let layer_list = VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::None)
        .children(hierarchy_rows.into_iter().map(|row| {
            row.into_stack_child()
                .height(LAYER_ROW_HEIGHT)
                .flex_shrink(0.0)
        }));
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

    let layer_actions = HStack::new()
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
                            canvas.selected_objects.clear();
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
                    selected_layer.is_some_and(|index| index + 1 < current_document.layers().len()),
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
        );

    let layer_properties = VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::Medium)
        .child(Text::new(format!(
            "Opacity — {:.0}%",
            bindings.layer_opacity.get() * 100.0
        )))
        .child(
            Slider::new(bindings.layer_opacity.binding())
                .range(0.0..=1.0)
                .step(0.01)
                .on_commit({
                    let document = document.clone();
                    move |opacity| {
                        document.update(|document| document.set_selected_layer_opacity(opacity));
                    }
                }),
        )
        .child(
            HStack::new()
                .gap(StackGap::ExtraSmall)
                .child(
                    icon_button::view("lock")
                        .style(if selected_layer_locked {
                            ButtonStyle::Standard
                        } else {
                            ButtonStyle::Ghost
                        })
                        .on_click({
                            let document = document.clone();
                            let canvas = canvas.clone();
                            move || {
                                if document.update(Document::toggle_selected_layer_lock) {
                                    clear_canvas_selection(&canvas);
                                }
                            }
                        }),
                )
                .child(
                    icon_button::view("grid-2x2-check")
                        .style(if selected_layer_alpha_locked {
                            ButtonStyle::Standard
                        } else {
                            ButtonStyle::Ghost
                        })
                        .on_click({
                            let document = document.clone();
                            move || {
                                document.update(Document::toggle_selected_layer_alpha_lock);
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
        );

    let layer_panel = VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::Medium)
        .child(layer_actions.into_stack_child().flex_shrink(0.0))
        .child(
            Scroll::new(bindings.layer_scroll.clone())
                .intrinsic_cross_axis(true)
                .scrollbar(ScrollBarVisibility::WhileScrolling)
                .content(layer_list.layout().height(layer_content_height))
                .layout()
                .height(0.0)
                .flex_grow(1.0),
        )
        .child(Divider::new())
        .child(layer_properties.into_stack_child().flex_shrink(0.0));

    let mut content = VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::Medium)
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

    let mut project_name = crate::project::display_name(bindings.project_path.get().as_deref());
    if current_document.is_modified() {
        project_name.push('*');
    }
    let tool_panel = VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::Medium)
        .child(
            Scroll::new(bindings.property_scroll.clone())
                .intrinsic_cross_axis(true)
                .scrollbar(ScrollBarVisibility::WhileScrolling)
                .content(content)
                .layout()
                .height(0.0)
                .flex_grow(1.0),
        );
    match palette {
        InspectorPalette::ToolProperty => Box::new(
            Padding::all(12.0).content(
                VStack::new()
                    .alignment(StackAlignment::Stretch)
                    .gap(StackGap::Medium)
                    .child(palette_tab("Tool Property"))
                    .child(Divider::new())
                    .child(tool_panel.layout().flex_grow(1.0)),
            ),
        ),
        InspectorPalette::Layers => Box::new(
            Padding::all(12.0).content(
                VStack::new()
                    .alignment(StackAlignment::Stretch)
                    .gap(StackGap::Medium)
                    .child(palette_tab("Layers"))
                    .child(Divider::new())
                    .child(layer_panel.layout().flex_grow(1.0))
                    .child(Divider::new())
                    .child(Text::new(project_name).into_stack_child().flex_shrink(0.0)),
            ),
        ),
    }
}

fn palette_tab(label: &'static str) -> impl View + 'static {
    HStack::new()
        .alignment(StackAlignment::Center)
        .gap(StackGap::None)
        .child(Button::new(label).style(ButtonStyle::Standard))
        .child(Spacer::new())
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
    canvas.selected_objects.clear();
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
        &[],
        None,
        true,
    )
    .ok()?;
    let svg = SvgData::decode(source.as_bytes()).ok()?;
    let image = ImageData::from_svg(&svg, 64, 64).ok()?;
    Some(Image::new(image))
}
