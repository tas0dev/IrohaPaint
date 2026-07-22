use viewkit::prelude::*;

use crate::canvas::{CanvasController, NavigatorCanvas};
use crate::document::Document;

pub fn view(
    document: State<Document>,
    canvas: CanvasController,
    view_revision: State<u64>,
) -> impl View + 'static {
    VStack::new()
        .alignment(StackAlignment::Stretch)
        .gap(StackGap::Medium)
        .child(
            NavigatorCanvas::new(document.clone(), canvas.clone())
                .layout()
                .height(0.0)
                .flex_grow(1.0),
        )
        .child(
            HStack::new()
                .gap(StackGap::ExtraSmall)
                .child(Button::new("Fit").on_click({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    let view_revision = view_revision.clone();
                    move || {
                        canvas.fit_canvas(&document.get());
                        view_revision.update(|revision| *revision = revision.wrapping_add(1));
                    }
                }))
                .child(Button::new("100%").on_click({
                    let canvas = canvas.clone();
                    let view_revision = view_revision.clone();
                    move || {
                        canvas.set_zoom(1.0);
                        view_revision.update(|revision| *revision = revision.wrapping_add(1));
                    }
                }))
                .child(
                    Button::new(if canvas.is_view_flipped() {
                        "Unflip"
                    } else {
                        "Flip"
                    })
                    .on_click(move || {
                        canvas.toggle_view_flip();
                        view_revision.update(|revision| *revision = revision.wrapping_add(1));
                    }),
                ),
        )
}
