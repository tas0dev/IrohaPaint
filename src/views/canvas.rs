use viewkit::prelude::*;

pub fn view() -> impl View + 'static {
    ZStack::new().alignment(ZStackAlignment::Center).child(
        Card::new()
            .content(Background::new())
            .radius(CornerRadius::None),
    )
}
