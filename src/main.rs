use viewkit::prelude::*;

struct IrohaPaint;

impl App for IrohaPaint {
    type Body = Box<dyn View + 'static>;

    fn new() -> Self {
        Self
    }

    fn window(&self) -> WindowOptions {
        WindowOptions::new("IrohaPaint")
            .size(1280.0, 800.0)
            .resizable(true)
    }

    fn body(&self, _context: &ViewContext) -> Box<dyn View + 'static> {
        Box::new(
            Background::new()
                .background(
                    Rectangle::new()
                        .color(RectangleColor::Custom(Color::from_rgb_hex(0xf2f2f4))),
                )
                .content(
                    VStack::new()
                        .alignment(StackAlignment::Center)
                        .child(
                            Text::new("IrohaPaint")
                        ),
                ),
        )
    }
}

fn main() -> Result<(), ViewKitError> {
    run::<IrohaPaint>()
}