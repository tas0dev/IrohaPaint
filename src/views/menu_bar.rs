use viewkit::prelude::*;

pub fn view() -> impl View + 'static {
    Padding::symmetric(8.0, 6.0).content(
        HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::Small)
            .child(Button::new("File").style(ButtonStyle::Ghost))
            .child(Button::new("Edit").style(ButtonStyle::Ghost))
            .child(Button::new("View").style(ButtonStyle::Ghost)),
    )
}
