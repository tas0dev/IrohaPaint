use viewkit::prelude::*;

use crate::brush::BrushLibrary;
use crate::canvas::{CanvasBindings, CanvasController, EditorCanvas};
use crate::document::{Document, ObjectId};
use crate::editor::EditorTool;
use crate::views::{inspector, menu_bar, settings_dialog, tool_bar};

pub struct IrohaPaint {
    active_tool: State<EditorTool>,
    document: State<Document>,
    canvas: CanvasController,
    export_status: State<String>,
    file_menu: PopupMenuState,
    pen_menu: PopupMenuState,
    document_settings: ModalState,
    brush_settings: ModalState,
    layer_name_settings: ModalState,
    brushes: State<BrushLibrary>,
    canvas_width: State<String>,
    canvas_height: State<String>,
    background_hex: State<String>,
    brush_name: State<String>,
    brush_status: State<String>,
    layer_name: State<String>,
    stroke_color: State<Color>,
    fill_color: State<Color>,
    color_target: State<usize>,
    brush_width: State<f32>,
    blob_width: State<f32>,
    paint_size: State<f32>,
    paint_opacity: State<f32>,
    paint_softness: State<f32>,
    eraser_mode: State<usize>,
    smoothing: State<f32>,
    inspected_object: State<Option<ObjectId>>,
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
            document_settings: ModalState::new(),
            brush_settings: ModalState::new(),
            layer_name_settings: ModalState::new(),
            brushes: State::new(BrushLibrary::default()),
            canvas_width: State::new(String::from("1200")),
            canvas_height: State::new(String::from("1200")),
            background_hex: State::new(String::from("#FFFFFFFF")),
            brush_name: State::new(String::from("Custom Brush")),
            brush_status: State::new(String::new()),
            layer_name: State::new(String::new()),
            stroke_color: State::new(Color::BLACK),
            fill_color: State::new(Color::TRANSPARENT),
            color_target: State::new(0),
            brush_width: State::new(2.5),
            blob_width: State::new(18.0),
            paint_size: State::new(48.0),
            paint_opacity: State::new(0.8),
            paint_softness: State::new(0.2),
            eraser_mode: State::new(0),
            smoothing: State::new(0.72),
            inspected_object: State::new(None),
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
                            CanvasBindings {
                                fill_color: self.fill_color.clone(),
                                blob_width: self.blob_width.clone(),
                                paint_size: self.paint_size.clone(),
                                paint_opacity: self.paint_opacity.clone(),
                                paint_softness: self.paint_softness.clone(),
                                eraser_mode: self.eraser_mode.clone(),
                            },
                        )
                        .layout()
                        .flex_grow(1.0),
                    )
                    .child(Divider::new())
                    .child(inspector::view(
                        self.document.clone(),
                        self.canvas.clone(),
                        self.brushes.clone(),
                        self.active_tool.clone(),
                        inspector::InspectorBindings {
                            stroke_color: self.stroke_color.clone(),
                            fill_color: self.fill_color.clone(),
                            color_target: self.color_target.clone(),
                            brush_width: self.brush_width.clone(),
                            blob_width: self.blob_width.clone(),
                            paint_size: self.paint_size.clone(),
                            paint_opacity: self.paint_opacity.clone(),
                            paint_softness: self.paint_softness.clone(),
                            eraser_mode: self.eraser_mode.clone(),
                            smoothing: self.smoothing.clone(),
                            inspected_object: self.inspected_object.clone(),
                            layer_name: self.layer_name.clone(),
                            layer_name_settings: self.layer_name_settings.clone(),
                        },
                    ))
                    .layout()
                    .flex_grow(1.0),
            );
        let menu = menu_bar::file_menu(
            self.document.clone(),
            self.canvas.clone(),
            self.export_status.clone(),
            self.document_settings.clone(),
        );
        let pen_menu = tool_bar::pen_menu(
            self.brushes.clone(),
            self.brush_settings.clone(),
            self.stroke_color.clone(),
            self.brush_width.clone(),
            self.blob_width.clone(),
            self.smoothing.clone(),
            self.active_tool.clone(),
        );
        let content = PopupMenuHost::new(content, pen_menu, self.pen_menu.clone());
        let content = PopupMenuHost::new(content, menu, self.file_menu.clone());
        let document_settings = settings_dialog::document_settings(
            self.document.clone(),
            settings_dialog::DocumentSettingsBindings {
                width: self.canvas_width.clone(),
                height: self.canvas_height.clone(),
                background: self.background_hex.clone(),
            },
            self.document_settings.clone(),
        );
        let brush_settings = settings_dialog::brush_settings(
            self.brushes.clone(),
            self.brush_name.clone(),
            self.brush_status.clone(),
            self.active_tool.clone(),
            self.blob_width.clone(),
            self.brush_settings.clone(),
        );
        let layer_name_settings = settings_dialog::layer_name_settings(
            self.document.clone(),
            self.layer_name.clone(),
            self.layer_name_settings.clone(),
        );
        let content = ModalHost::new(content, document_settings, self.document_settings.clone());
        let content = ModalHost::new(content, brush_settings, self.brush_settings.clone());
        Box::new(ModalHost::new(
            content,
            layer_name_settings,
            self.layer_name_settings.clone(),
        ))
    }
}
