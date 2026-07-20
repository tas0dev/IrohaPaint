use viewkit::prelude::*;

use crate::brush::{BrushLibrary, BrushTip};
use crate::canvas::CanvasController;
use crate::document::{CanvasSize, Document, DocumentColor, NodeKind};

pub struct InspectorBindings {
    pub canvas_width: State<String>,
    pub canvas_height: State<String>,
    pub background_hex: State<String>,
    pub stroke_hex: State<String>,
    pub brush_name: State<String>,
}

pub fn view(
    document: State<Document>,
    canvas: CanvasController,
    brushes: State<BrushLibrary>,
    bindings: InspectorBindings,
) -> impl View + 'static {
    let InspectorBindings {
        canvas_width,
        canvas_height,
        background_hex,
        stroke_hex,
        brush_name,
    } = bindings;
    let current_document = document.get();
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

    Padding::all(12.0).content(
        VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::Medium)
            .child(Text::new("Layers").weight(700))
            .child(Divider::new())
            .children(layer_rows)
            .child(Text::new("Properties").weight(700))
            .child(Divider::new())
            .child(Text::new("Document").weight(700))
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(TextField::new(canvas_width.binding()).placeholder("Width"))
                    .child(TextField::new(canvas_height.binding()).placeholder("Height")),
            )
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(Button::new("Apply Size").on_click({
                        let document = document.clone();
                        let width = canvas_width.clone();
                        let height = canvas_height.clone();
                        move || {
                            let parsed = width
                                .get()
                                .parse::<f32>()
                                .ok()
                                .zip(height.get().parse::<f32>().ok());
                            if let Some((width, height)) = parsed {
                                document.update(|document| {
                                    document.set_canvas_size(CanvasSize::Custom { width, height })
                                });
                            }
                        }
                    }))
                    .child(Button::new("Fit Artwork").on_click({
                        let document = document.clone();
                        move || {
                            document
                                .update(|document| document.set_canvas_size(CanvasSize::FitArtwork))
                        }
                    })),
            )
            .child(TextField::new(background_hex.binding()).placeholder("#RRGGBBAA"))
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(Button::new("Set Background").on_click({
                        let document = document.clone();
                        let background_hex = background_hex.clone();
                        move || {
                            if let Some(color) = DocumentColor::from_hex(&background_hex.get()) {
                                document.update(|document| document.set_background(color));
                            }
                        }
                    }))
                    .child(Button::new("Transparent").on_click({
                        let document = document.clone();
                        move || {
                            document.update(|document| {
                                document.set_background(DocumentColor::TRANSPARENT)
                            })
                        }
                    })),
            )
            .child(Text::new("Brush").weight(700))
            .child(TextField::new(brush_name.binding()).placeholder("Preset Name"))
            .child(TextField::new(stroke_hex.binding()).placeholder("Stroke #RRGGBBAA"))
            .child(Text::new(format!(
                "{} — {:.1} px",
                brushes.get().active().name,
                brushes.get().active().width
            )))
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(brush_button("Thinner", brushes.clone(), |brush| {
                        brush.width -= 0.5
                    }))
                    .child(brush_button("Thicker", brushes.clone(), |brush| {
                        brush.width += 0.5
                    }))
                    .child(brush_button("Smoother", brushes.clone(), |brush| {
                        brush.smoothing += 0.1
                    }))
                    .child(brush_button("More Taper", brushes.clone(), |brush| {
                        brush.taper_start += 0.1;
                        brush.taper_end += 0.1;
                    })),
            )
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(brush_button("Less Smooth", brushes.clone(), |brush| {
                        brush.smoothing -= 0.1;
                    }))
                    .child(brush_button("Less Taper", brushes.clone(), |brush| {
                        brush.taper_start -= 0.1;
                        brush.taper_end -= 0.1;
                    }))
                    .child(brush_button("Flatter Tip", brushes.clone(), |brush| {
                        if let BrushTip::Ellipse { roundness, .. } = &mut brush.tip {
                            *roundness -= 0.1;
                        }
                    }))
                    .child(brush_button("Rounder Tip", brushes.clone(), |brush| {
                        if let BrushTip::Ellipse { roundness, .. } = &mut brush.tip {
                            *roundness += 0.1;
                        }
                    })),
            )
            .child(
                HStack::new()
                    .gap(StackGap::ExtraSmall)
                    .child(brush_button("Round Tip", brushes.clone(), |brush| {
                        brush.tip = BrushTip::Round
                    }))
                    .child(brush_button("Ellipse Tip", brushes.clone(), |brush| {
                        brush.tip = BrushTip::Ellipse {
                            roundness: 0.75,
                            angle: -45.0,
                        };
                    }))
                    .child(brush_button("Rotate Tip", brushes.clone(), |brush| {
                        if let BrushTip::Ellipse { angle, .. } = &mut brush.tip {
                            *angle += 15.0;
                        }
                    }))
                    .child(Button::new("Save Preset").on_click({
                        let brushes = brushes.clone();
                        let brush_name = brush_name.clone();
                        move || brushes.update(|library| library.save_active_as(&brush_name.get()))
                    })),
            )
            .child(Button::new("Set Brush Color").on_click({
                let brushes = brushes.clone();
                let stroke_hex = stroke_hex.clone();
                let document = document.clone();
                move || {
                    if let Some(color) = DocumentColor::from_hex(&stroke_hex.get()) {
                        brushes
                            .update(|library| library.update_active(|brush| brush.color = color));
                        document.update(|document| document.set_selected_stroke_color(color));
                    }
                }
            }))
            .child(Text::new("Node Editing").weight(700))
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
            )
            .child(Text::new(
                "Click a curve to add a node without changing its shape.",
            )),
    )
}

fn brush_button(
    title: &'static str,
    brushes: State<BrushLibrary>,
    update: impl Fn(&mut crate::brush::BrushDefinition) + 'static,
) -> Button {
    Button::new(title).on_click(move || {
        brushes.update(|library| library.update_active(|brush| update(brush)));
    })
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
