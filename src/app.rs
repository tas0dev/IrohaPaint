use viewkit::prelude::*;

use crate::brush::BrushLibrary;
use crate::canvas::{CanvasController, EditorCanvas};
use crate::document::Document;
use crate::editor::EditorTool;
use crate::views::{inspector, menu_bar, tool_bar};

pub struct IrohaPaint {
    active_tool: State<EditorTool>,
    document: State<Document>,
    canvas: CanvasController,
    export_status: State<String>,
    file_menu: PopupMenuState,
    pen_menu: PopupMenuState,
    brushes: State<BrushLibrary>,
    canvas_width: State<String>,
    canvas_height: State<String>,
    background_hex: State<String>,
    stroke_hex: State<String>,
    brush_name: State<String>,
}

impl App for IrohaPaint {
    type Body = Box<dyn View + 'static>;

    fn new() -> Self {
        Self {
            active_tool: State::new(EditorTool::Select),
            document: State::new(Document::new()),
            canvas: CanvasController::new(),
            export_status: State::new(String::new()),
            file_menu: PopupMenuState::new(),
            pen_menu: PopupMenuState::new(),
            brushes: State::new(BrushLibrary::default()),
            canvas_width: State::new(String::from("1200")),
            canvas_height: State::new(String::from("1200")),
            background_hex: State::new(String::from("#00000000")),
            stroke_hex: State::new(String::from("#000000FF")),
            brush_name: State::new(String::from("Custom Brush")),
        }
    }

    fn window(&self) -> WindowOptions {
        WindowOptions::new("IrohaPaint")
            .size(1280.0, 800.0)
            .resizable(true)
    }

    fn body(&self, _context: &ViewContext) -> Box<dyn View + 'static> {
        let content = VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::None)
            .child(menu_bar::view(
                self.document.clone(),
                self.export_status.clone(),
                self.file_menu.clone(),
            ))
            .child(Divider::new())
            .child(
                HStack::new()
                    .alignment(StackAlignment::Stretch)
                    .gap(StackGap::None)
                    .child(tool_bar::view(
                        self.active_tool.clone(),
                        self.pen_menu.clone(),
                    ))
                    .child(Divider::new())
                    .child(
                        EditorCanvas::new(
                            self.document.clone(),
                            self.active_tool.clone(),
                            self.canvas.clone(),
                            self.brushes.clone(),
                        )
                        .layout()
                        .flex_grow(1.0),
                    )
                    .child(Divider::new())
                    .child(inspector::view(
                        self.document.clone(),
                        self.canvas.clone(),
                        self.brushes.clone(),
                        inspector::InspectorBindings {
                            canvas_width: self.canvas_width.clone(),
                            canvas_height: self.canvas_height.clone(),
                            background_hex: self.background_hex.clone(),
                            stroke_hex: self.stroke_hex.clone(),
                            brush_name: self.brush_name.clone(),
                        },
                    ))
                    .layout()
                    .flex_grow(1.0),
            );
        let menu = menu_bar::file_menu(self.document.clone(), self.export_status.clone());
        let pen_menu = tool_bar::pen_menu(self.brushes.clone());
        let content = PopupMenuHost::new(content, pen_menu, self.pen_menu.clone());
        Box::new(PopupMenuHost::new(content, menu, self.file_menu.clone()))
    }
}
