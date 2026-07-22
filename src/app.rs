use viewkit::prelude::*;

use crate::brush::BrushLibrary;
use crate::canvas::{CanvasBindings, CanvasController, EditorCanvas};
use crate::document::{Document, FolderId, ObjectId};
use crate::editor::EditorTool;
use crate::project;
use crate::views::{inspector, menu_bar, navigator, settings_dialog, tool_bar};
use std::path::PathBuf;

pub struct IrohaPaint {
    active_tool: State<EditorTool>,
    document: State<Document>,
    canvas: CanvasController,
    export_status: State<String>,
    project_path: State<Option<PathBuf>>,
    file_menu: PopupMenuState,
    edit_menu: PopupMenuState,
    view_menu: PopupMenuState,
    pen_menu: PopupMenuState,
    document_settings: ModalState,
    brush_settings: ModalState,
    layer_name_settings: ModalState,
    folder_name_settings: ModalState,
    brushes: State<BrushLibrary>,
    canvas_width: State<String>,
    canvas_height: State<String>,
    background_hex: State<String>,
    brush_name: State<String>,
    brush_status: State<String>,
    layer_name: State<String>,
    folder_name: State<String>,
    editing_folder: State<Option<FolderId>>,
    layer_opacity: State<f32>,
    inspected_layer: State<Option<usize>>,
    layer_scroll: ScrollState,
    property_scroll: ScrollState,
    right_palette_tab: State<usize>,
    view_revision: State<u64>,
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
            project_path: State::new(None),
            file_menu: PopupMenuState::new(),
            edit_menu: PopupMenuState::new(),
            view_menu: PopupMenuState::new(),
            pen_menu: PopupMenuState::new(),
            document_settings: ModalState::new(),
            brush_settings: ModalState::new(),
            layer_name_settings: ModalState::new(),
            folder_name_settings: ModalState::new(),
            brushes: State::new(BrushLibrary::default()),
            canvas_width: State::new(String::from("1200")),
            canvas_height: State::new(String::from("1200")),
            background_hex: State::new(String::from("#FFFFFFFF")),
            brush_name: State::new(String::from("Custom Brush")),
            brush_status: State::new(String::new()),
            layer_name: State::new(String::new()),
            folder_name: State::new(String::new()),
            editing_folder: State::new(None),
            layer_opacity: State::new(1.0),
            inspected_layer: State::new(None),
            layer_scroll: ScrollState::new(),
            property_scroll: ScrollState::new(),
            right_palette_tab: State::new(1),
            view_revision: State::new(0),
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

    fn should_close(&self) -> bool {
        match project::prepare_to_replace(&self.document, &self.project_path) {
            Ok(should_close) => should_close,
            Err(error) => {
                self.export_status.set(format!("Save failed: {error}"));
                false
            }
        }
    }

    fn body(&self, _context: &ViewContext) -> Box<dyn View + 'static> {
        let _view_revision = self.view_revision.get();
        let inspector_bindings = || inspector::InspectorBindings {
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
            folder_name: self.folder_name.clone(),
            editing_folder: self.editing_folder.clone(),
            folder_name_settings: self.folder_name_settings.clone(),
            layer_opacity: self.layer_opacity.clone(),
            inspected_layer: self.inspected_layer.clone(),
            project_path: self.project_path.clone(),
            layer_scroll: self.layer_scroll.clone(),
            property_scroll: self.property_scroll.clone(),
        };
        let right_panel: Box<dyn View> = if self.right_palette_tab.get() == 0 {
            Box::new(Padding::all(12.0).content(navigator::view(
                self.document.clone(),
                self.canvas.clone(),
                self.view_revision.clone(),
            )))
        } else {
            Box::new(inspector::view(
                self.document.clone(),
                self.canvas.clone(),
                self.brushes.clone(),
                self.active_tool.clone(),
                inspector_bindings(),
                inspector::InspectorPalette::Layers,
            ))
        };
        let right_dock = VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::None)
            .child(
                Padding::symmetric(12.0, 8.0)
                    .content(
                        SegmentedControl::new(self.right_palette_tab.binding())
                            .item(0, "Navigator")
                            .item(1, "Layers"),
                    )
                    .into_stack_child()
                    .flex_shrink(0.0),
            )
            .child(Divider::new())
            .child(right_panel.layout().height(0.0).flex_grow(1.0));
        let content = VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::None)
            .child(
                menu_bar::view(
                    self.document.clone(),
                    self.canvas.clone(),
                    self.export_status.clone(),
                    self.file_menu.clone(),
                    self.edit_menu.clone(),
                    self.view_menu.clone(),
                )
                .into_stack_child()
                .flex_shrink(0.0),
            )
            .child(Divider::new())
            .child(
                HStack::new()
                    .alignment(StackAlignment::Stretch)
                    .gap(StackGap::None)
                    .child(
                        tool_bar::view(self.active_tool.clone(), self.pen_menu.clone())
                            .into_stack_child()
                            .flex_shrink(0.0),
                    )
                    .child(Divider::new())
                    .child(
                        inspector::view(
                            self.document.clone(),
                            self.canvas.clone(),
                            self.brushes.clone(),
                            self.active_tool.clone(),
                            inspector_bindings(),
                            inspector::InspectorPalette::ToolProperty,
                        )
                        .layout()
                        .width(248.0)
                        .flex_shrink(0.0),
                    )
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
                        .width(0.0)
                        .flex_grow(1.0),
                    )
                    .child(Divider::new())
                    .child(right_dock.layout().width(248.0).flex_shrink(0.0))
                    .layout()
                    .height(0.0)
                    .flex_grow(1.0),
            );
        let menu = menu_bar::file_menu(
            self.document.clone(),
            self.canvas.clone(),
            self.export_status.clone(),
            self.project_path.clone(),
            self.document_settings.clone(),
            menu_bar::DocumentFieldStates {
                width: self.canvas_width.clone(),
                height: self.canvas_height.clone(),
                background: self.background_hex.clone(),
            },
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
        let edit_menu = menu_bar::edit_menu(
            self.document.clone(),
            self.canvas.clone(),
            self.active_tool.clone(),
        );
        let view_menu = menu_bar::view_menu(
            self.document.clone(),
            self.canvas.clone(),
            self.right_palette_tab.clone(),
            self.view_revision.clone(),
        );
        let content = PopupMenuHost::new(content, pen_menu, self.pen_menu.clone());
        let content = PopupMenuHost::new(content, edit_menu, self.edit_menu.clone());
        let content = PopupMenuHost::new(content, view_menu, self.view_menu.clone());
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
        let folder_name_settings = settings_dialog::folder_name_settings(
            self.document.clone(),
            self.folder_name.clone(),
            self.editing_folder.clone(),
            self.folder_name_settings.clone(),
        );
        let content = ModalHost::new(content, document_settings, self.document_settings.clone());
        let content = ModalHost::new(content, brush_settings, self.brush_settings.clone());
        let content = ModalHost::new(
            content,
            layer_name_settings,
            self.layer_name_settings.clone(),
        );
        Box::new(ModalHost::new(
            content,
            folder_name_settings,
            self.folder_name_settings.clone(),
        ))
    }
}
