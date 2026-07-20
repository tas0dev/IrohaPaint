use viewkit::prelude::*;

use crate::document::Document;

pub fn view(document: State<Document>) -> impl View + 'static {
    let current_document = document.get();
    Padding::symmetric(8.0, 6.0).content(
        HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::Small)
            .child(Button::new("File").style(ButtonStyle::Ghost))
            .child(Button::new("Edit").style(ButtonStyle::Ghost))
            .child(Button::new("View").style(ButtonStyle::Ghost))
            .child(
                Button::new("Undo")
                    .style(ButtonStyle::Ghost)
                    .enabled(current_document.can_undo())
                    .on_click({
                        let document = document.clone();
                        move || document.update(Document::undo)
                    }),
            )
            .child(
                Button::new("Redo")
                    .style(ButtonStyle::Ghost)
                    .enabled(current_document.can_redo())
                    .on_click(move || document.update(Document::redo)),
            ),
    )
}
