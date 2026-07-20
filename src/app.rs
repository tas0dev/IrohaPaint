use viewkit::prelude::*;

use crate::canvas::{CanvasController, EditorCanvas};
use crate::document::Document;
use crate::editor::EditorTool;
use crate::views::{inspector, menu_bar, tool_bar};

pub struct IrohaPaint {
    active_tool: State<EditorTool>,
    document: State<Document>,
    canvas: CanvasController,
}

impl App for IrohaPaint {
    type Body = Box<dyn View + 'static>;

    fn new() -> Self {
        Self {
            active_tool: State::new(EditorTool::Select),
            document: State::new(Document::new()),
            canvas: CanvasController::new(),
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
                .child(menu_bar::view(self.document.clone()))
                .child(Divider::new())
                .child(
                    HStack::new()
                        .alignment(StackAlignment::Stretch)
                        .gap(StackGap::None)
                        .child(tool_bar::view(self.active_tool.clone()))
                        .child(Divider::new())
                        .child(
                            EditorCanvas::new(
                                self.document.clone(),
                                self.active_tool.clone(),
                                self.canvas.clone(),
                            )
                            .layout()
                            .flex_grow(1.0),
                        )
                        .child(Divider::new())
                        .child(inspector::view(self.document.clone(), self.canvas.clone()))
                        .layout()
                        .flex_grow(1.0),
                ),
        )
    }
}
