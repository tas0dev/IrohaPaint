use viewkit::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorTool {
    Select,
    Pen,
    Rectangle,
    Ellipse,
}

struct IrohaPaint {
    active_tool: State<EditorTool>,
}

impl IrohaPaint {
    fn menu_bar(&self) -> impl View + 'static {
        Padding::symmetric(8.0, 6.0).content(
            HStack::new()
                .alignment(StackAlignment::Center)
                .gap(StackGap::Small)
                .child(
                    Button::new("File")
                        .style(ButtonStyle::Ghost),
                )
                .child(
                    Button::new("Edit")
                        .style(ButtonStyle::Ghost),
                )
                .child(
                    Button::new("View")
                        .style(ButtonStyle::Ghost),
                ),
        )
    }

    fn tool_bar(&self) -> impl View + 'static {
        let active_tool = self.active_tool.get();

        Padding::all(8.0).content(
            VStack::new()
                .alignment(StackAlignment::Stretch)
                .gap(StackGap::ExtraSmall)
                .child(
                    ListRow::new("Select")
                        .selected(active_tool == EditorTool::Select)
                        .on_select({
                            let active_tool = self.active_tool.clone();

                            move || {
                                active_tool.set(EditorTool::Select);
                            }
                        }),
                )
                .child(
                    ListRow::new("Pen")
                        .selected(active_tool == EditorTool::Pen)
                        .on_select({
                            let active_tool = self.active_tool.clone();

                            move || {
                                active_tool.set(EditorTool::Pen);
                            }
                        }),
                )
                .child(
                    ListRow::new("Rectangle")
                        .selected(active_tool == EditorTool::Rectangle)
                        .on_select({
                            let active_tool = self.active_tool.clone();

                            move || {
                                active_tool.set(EditorTool::Rectangle);
                            }
                        }),
                )
                .child(
                    ListRow::new("Ellipse")
                        .selected(active_tool == EditorTool::Ellipse)
                        .on_select({
                            let active_tool = self.active_tool.clone();

                            move || {
                                active_tool.set(EditorTool::Ellipse);
                            }
                        }),
                ),
        )
    }

    fn canvas(&self) -> impl View + 'static {
        ZStack::new()
            .alignment(ZStackAlignment::Center)
            .child(
                Card::new()
                    .content(
                        Background::new()
                    )
                    .radius(CornerRadius::None)
            )
    }

    fn inspector(&self) -> impl View + 'static {
        Padding::all(12.0).content(
            VStack::new()
                .alignment(StackAlignment::Stretch)
                .gap(StackGap::Medium)
                .child(
                    Text::new("Layers")
                        .weight(700),
                )
                .child(Divider::new())
                .child(Text::new("There are no layers"))
                .child(
                    Text::new("Properties")
                        .weight(700),
                )
                .child(Divider::new())
                .child(Text::new("Please select an object")),
        )
    }
}

impl App for IrohaPaint {
    type Body = Box<dyn View + 'static>;

    fn new() -> Self {
        Self {
            active_tool: State::new(EditorTool::Select),
        }
    }

    fn window(&self) -> WindowOptions {
        WindowOptions::new("IrohaPaint")
            .size(1280.0, 800.0)
            .resizable(true)
    }

    fn body(&self, _context: &ViewContext) -> Box<dyn View + 'static> {
        Box::new(
            VStack::new()
                .alignment(StackAlignment::Stretch)
                .gap(StackGap::None)
                .child(self.menu_bar())
                .child(Divider::new())
                .child(
                    HStack::new()
                        .alignment(StackAlignment::Stretch)
                        .gap(StackGap::None)
                        .child(self.tool_bar())
                        .child(Divider::new())
                        .child(
                            self.canvas()
                                .layout()
                                .flex_grow(1.0),
                        )
                        .child(Divider::new())
                        .child(self.inspector())
                        .layout()
                        .flex_grow(1.0),
                ),
        )
    }
}

fn main() -> Result<(), ViewKitError> {
    run::<IrohaPaint>()
}