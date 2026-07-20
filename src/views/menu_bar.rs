use viewkit::prelude::*;

use crate::document::Document;
use crate::export;

pub fn view(
    document: State<Document>,
    export_status: State<String>,
    file_menu: PopupMenuState,
) -> impl View + 'static {
    let current_document = document.get();
    let status = export_status.get();
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
            .child(Text::new(status)),
    )
}

pub fn file_menu(
    document: State<Document>,
    export_status: State<String>,
    document_settings: ModalState,
) -> Menu {
    Menu::new()
        .item(MenuItem::new("Document Properties…").on_select(move || {
            document_settings.open();
        }))
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
