use viewkit::prelude::*;

use crate::document::Document;

pub fn view(document: State<Document>) -> impl View + 'static {
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
            .child(Text::new("Select an object to edit its properties")),
    )
}
