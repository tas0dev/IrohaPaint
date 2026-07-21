use std::path::PathBuf;
use viewkit::prelude::*;

use crate::document::{CanvasSize, Document};
use crate::editor::EditorTool;
use crate::export;
use crate::{canvas::CanvasController, project, reference};

#[derive(Clone)]
pub struct DocumentFieldStates {
    pub width: State<String>,
    pub height: State<String>,
    pub background: State<String>,
}

pub fn view(
    document: State<Document>,
    canvas: CanvasController,
    export_status: State<String>,
    file_menu: PopupMenuState,
    edit_menu: PopupMenuState,
) -> impl View + 'static {
    let current_document = document.get();
    let status = export_status.get();
    Padding::symmetric(8.0, 6.0).content(
        HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::Small)
            .child(PopupMenuButton::new("File", file_menu).style(ButtonStyle::Ghost))
            .child(PopupMenuButton::new("Edit", edit_menu).style(ButtonStyle::Ghost))
            .child(Button::new("View").style(ButtonStyle::Ghost))
            .child(
                Button::new("Undo")
                    .style(ButtonStyle::Ghost)
                    .enabled(current_document.can_undo())
                    .on_click({
                        let document = document.clone();
                        let canvas = canvas.clone();
                        move || {
                            document.update(Document::undo);
                            canvas.clear_selection();
                        }
                    }),
            )
            .child(
                Button::new("Redo")
                    .style(ButtonStyle::Ghost)
                    .enabled(current_document.can_redo())
                    .on_click(move || {
                        document.update(Document::redo);
                        canvas.clear_selection();
                    }),
            )
            .child(Text::new(status)),
    )
}

pub fn edit_menu(
    document: State<Document>,
    canvas: CanvasController,
    active_tool: State<EditorTool>,
) -> Menu {
    let has_selection = canvas.selection_count() > 0;
    Menu::new()
        .item(
            MenuItem::new("Select All")
                .shortcut("Ctrl/Cmd+A")
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    let active_tool = active_tool.clone();
                    move || {
                        document.update(|document| canvas.select_all_objects(document));
                        active_tool.set(EditorTool::Select);
                    }
                }),
        )
        .separator()
        .item(
            MenuItem::new("Copy")
                .shortcut("Ctrl/Cmd+C")
                .enabled(has_selection)
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        canvas.copy_selection(&document.get());
                    }
                }),
        )
        .item(MenuItem::new("Paste").shortcut("Ctrl/Cmd+V").on_select({
            let document = document.clone();
            let canvas = canvas.clone();
            let active_tool = active_tool.clone();
            move || {
                document.update(|document| canvas.paste(document));
                active_tool.set(EditorTool::Select);
            }
        }))
        .item(
            MenuItem::new("Duplicate")
                .shortcut("Ctrl/Cmd+D")
                .enabled(has_selection)
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    let active_tool = active_tool.clone();
                    move || {
                        document.update(|document| canvas.duplicate_selection(document));
                        active_tool.set(EditorTool::Select);
                    }
                }),
        )
        .separator()
        .item(
            MenuItem::new("Flip Horizontally")
                .enabled(has_selection)
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        document.update(|document| canvas.flip_selection(document, true));
                    }
                }),
        )
        .item(
            MenuItem::new("Flip Vertically")
                .enabled(has_selection)
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        document.update(|document| canvas.flip_selection(document, false));
                    }
                }),
        )
        .separator()
        .item(
            MenuItem::new("Bring Forward")
                .enabled(has_selection)
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        document.update(|document| canvas.move_selection_forward(document));
                    }
                }),
        )
        .item(
            MenuItem::new("Send Backward")
                .enabled(has_selection)
                .on_select({
                    let document = document.clone();
                    let canvas = canvas.clone();
                    move || {
                        document.update(|document| canvas.move_selection_backward(document));
                    }
                }),
        )
        .separator()
        .item(
            MenuItem::new("Delete")
                .shortcut("Delete")
                .enabled(has_selection)
                .on_select(move || {
                    document.update(|document| canvas.delete_selection(document));
                }),
        )
}

pub fn file_menu(
    document: State<Document>,
    canvas: CanvasController,
    export_status: State<String>,
    project_path: State<Option<PathBuf>>,
    document_settings: ModalState,
    document_fields: DocumentFieldStates,
) -> Menu {
    Menu::new()
        .item(MenuItem::new("New").on_select({
            let document = document.clone();
            let canvas = canvas.clone();
            let export_status = export_status.clone();
            let project_path = project_path.clone();
            let fields = document_fields.clone();
            move || match project::prepare_to_replace(&document, &project_path) {
                Ok(true) => {
                    let fresh = Document::new();
                    sync_document_fields(&fresh, &fields);
                    document.set(fresh);
                    project_path.set(None);
                    canvas.reset_for_document();
                    export_status.set(String::from("New project"));
                }
                Ok(false) => {}
                Err(error) => export_status.set(format!("Save failed: {error}")),
            }
        }))
        .item(MenuItem::new("Open…").on_select({
            let document = document.clone();
            let canvas = canvas.clone();
            let export_status = export_status.clone();
            let project_path = project_path.clone();
            let fields = document_fields.clone();
            move || match project::prepare_to_replace(&document, &project_path) {
                Ok(true) => match project::open_with_dialog() {
                    Ok(Some((opened, path))) => {
                        sync_document_fields(&opened, &fields);
                        document.set(opened);
                        project_path.set(Some(path));
                        canvas.reset_for_document();
                        export_status.set(String::from("Project opened"));
                    }
                    Ok(None) => {}
                    Err(error) => export_status.set(format!("Open failed: {error}")),
                },
                Ok(false) => {}
                Err(error) => export_status.set(format!("Save failed: {error}")),
            }
        }))
        .item(MenuItem::new("Save").on_select({
            let document = document.clone();
            let project_path = project_path.clone();
            let export_status = export_status.clone();
            move || match project::save_current(&document, &project_path, false) {
                Ok(true) => export_status.set(String::from("Project saved")),
                Ok(false) => {}
                Err(error) => export_status.set(format!("Save failed: {error}")),
            }
        }))
        .item(MenuItem::new("Save As…").on_select({
            let document = document.clone();
            let project_path = project_path.clone();
            let export_status = export_status.clone();
            move || match project::save_current(&document, &project_path, true) {
                Ok(true) => export_status.set(String::from("Project saved")),
                Ok(false) => {}
                Err(error) => export_status.set(format!("Save failed: {error}")),
            }
        }))
        .separator()
        .item(MenuItem::new("Document Properties…").on_select(move || {
            document_settings.open();
        }))
        .item(MenuItem::new("Import Reference Image…").on_select({
            let document = document.clone();
            let canvas = canvas.clone();
            let export_status = export_status.clone();
            move || match reference::import_with_dialog(&canvas, &document.get()) {
                Ok(true) => export_status.set(String::from("Reference image imported")),
                Ok(false) => {}
                Err(error) => export_status.set(format!("Import failed: {error}")),
            }
        }))
        .item(
            MenuItem::new("Remove Reference Image")
                .enabled(canvas.has_reference_image())
                .on_select(move || canvas.remove_reference_image()),
        )
        .separator()
        .item(export_item(
            "Export SVG",
            export::ExportFormat::Svg,
            document.clone(),
            export_status.clone(),
        ))
        .separator()
        .item(export_item(
            "Export PNG 1x",
            export::ExportFormat::Png { scale: 1 },
            document.clone(),
            export_status.clone(),
        ))
        .item(export_item(
            "Export PNG 2x",
            export::ExportFormat::Png { scale: 2 },
            document.clone(),
            export_status.clone(),
        ))
        .item(export_item(
            "Export PNG 4x",
            export::ExportFormat::Png { scale: 4 },
            document,
            export_status,
        ))
}

fn sync_document_fields(document: &Document, fields: &DocumentFieldStates) {
    match document.properties().canvas_size {
        CanvasSize::FitArtwork => {
            fields.width.set(String::new());
            fields.height.set(String::new());
        }
        CanvasSize::Custom { width, height } => {
            fields.width.set(format!("{width:.0}"));
            fields.height.set(format!("{height:.0}"));
        }
    }
    fields
        .background
        .set(document.properties().background.to_hex());
}

fn export_item(
    label: &'static str,
    format: export::ExportFormat,
    document: State<Document>,
    export_status: State<String>,
) -> MenuItem {
    MenuItem::new(label).on_select(move || {
        match export::export_with_dialog(&document.get(), format) {
            Ok(true) => export_status.set(format!("Exported {}", format.label())),
            Ok(false) => {}
            Err(error) => export_status.set(format!("Export failed: {error}")),
        }
    })
}
