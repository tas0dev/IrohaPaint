use std::path::PathBuf;
use viewkit::prelude::*;

use crate::document::{CanvasSize, Document};
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
    export_status: State<String>,
    project_path: State<Option<PathBuf>>,
    file_menu: PopupMenuState,
) -> impl View + 'static {
    let current_document = document.get();
    let status = export_status.get();
    let mut project_name = project::display_name(project_path.get().as_deref());
    if current_document.is_modified() {
        project_name.push('*');
    }
    Padding::symmetric(8.0, 6.0).content(
        HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::Small)
            .child(PopupMenuButton::new("File", file_menu).style(ButtonStyle::Ghost))
            .child(Button::new("Edit").style(ButtonStyle::Ghost))
            .child(Button::new("View").style(ButtonStyle::Ghost))
            .child(
                Button::new("Undo")
                    .style(ButtonStyle::Ghost)
                    .enabled(current_document.can_undo())
                    .on_click({
                        let document = document.clone();
                        move || document.update(Document::undo)
                    }),
            )
            .child(
                Button::new("Redo")
                    .style(ButtonStyle::Ghost)
                    .enabled(current_document.can_redo())
                    .on_click(move || document.update(Document::redo)),
            )
            .child(Text::new(project_name))
            .child(Text::new(status)),
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
