use viewkit::prelude::*;

use crate::canvas::CanvasController;
use crate::document::{Document, NodeKind};

pub fn view(document: State<Document>, canvas: CanvasController) -> impl View + 'static {
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
