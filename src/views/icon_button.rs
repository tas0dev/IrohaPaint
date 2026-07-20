use viewkit::prelude::*;

use crate::icons;

pub fn view(name: &str) -> Button {
    let icon = icons::icon(name).unwrap_or_else(|| panic!("icon `{name}` was not found"));

    Button::new("")
        .content(Svg::new(icon))
        .style(ButtonStyle::Ghost)
}
