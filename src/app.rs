use viewkit::prelude::*;

use crate::document::Document;
use crate::editor::EditorTool;
use crate::views::{canvas, inspector, menu_bar, tool_bar};

pub struct IrohaPaint {
    active_tool: State<EditorTool>,
    document: State<Document>,
}

impl App for IrohaPaint {
    type Body = Box<dyn View + 'static>;

    fn new() -> Self {
        Self {
            active_tool: State::new(EditorTool::Select),
            document: State::new(Document::new()),
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
                .child(menu_bar::view())
                .child(Divider::new())
                .child(
                    HStack::new()
                        .alignment(StackAlignment::Stretch)
                        .gap(StackGap::None)
                        .child(tool_bar::view(self.active_tool.clone()))
                        .child(Divider::new())
                        .child(canvas::view().layout().flex_grow(1.0))
                        .child(Divider::new())
                        .child(inspector::view(self.document.clone()))
                        .layout()
                        .flex_grow(1.0),
                ),
        )
    }
}
