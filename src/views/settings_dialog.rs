use viewkit::prelude::*;

use super::icon_button;
use crate::brush::{BrushDefinition, BrushKind, BrushLibrary, BrushTip};
use crate::document::{CanvasSize, Document, DocumentColor};
use crate::editor::EditorTool;

pub struct DocumentSettingsBindings {
    pub width: State<String>,
    pub height: State<String>,
    pub background: State<String>,
}

pub fn layer_name_settings(
    document: State<Document>,
    name: State<String>,
    modal: ModalState,
) -> impl View + 'static {
    Card::new().content(
        Padding::all(16.0).content(
            VStack::new()
                .alignment(StackAlignment::Stretch)
                .gap(StackGap::Medium)
                .child(Text::new("Rename Layer").weight(700))
                .child(Divider::new())
                .child(TextField::new(name.binding()).placeholder("Layer Name"))
                .child(
                    HStack::new()
                        .gap(StackGap::Small)
                        .child(
                            icon_button::view("check")
                                .style(ButtonStyle::Primary)
                                .on_click({
                                    let document = document.clone();
                                    let name = name.clone();
                                    let modal = modal.clone();
                                    move || {
                                        if document.update(|document| {
                                            document.rename_selected_layer(&name.get())
                                        }) {
                                            modal.close();
                                        }
                                    }
                                }),
                        )
                        .child(icon_button::view("x").on_click(move || modal.close())),
                ),
        ),
    )
}

pub fn document_settings(
    document: State<Document>,
    bindings: DocumentSettingsBindings,
    modal: ModalState,
) -> impl View + 'static {
    let DocumentSettingsBindings {
        width,
        height,
        background,
    } = bindings;

    Card::new().content(
        Padding::all(16.0).content(
            VStack::new()
                .alignment(StackAlignment::Stretch)
                .gap(StackGap::Medium)
                .child(Text::new("Document Properties").weight(700))
                .child(Divider::new())
                .child(Text::new("Canvas Size").weight(700))
                .child(
                    HStack::new()
                        .gap(StackGap::Small)
                        .child(TextField::new(width.binding()).placeholder("Width"))
                        .child(TextField::new(height.binding()).placeholder("Height")),
                )
                .child(icon_button::view("check").on_click({
                    let document = document.clone();
                    let width = width.clone();
                    let height = height.clone();
                    move || {
                        if let Some((width, height)) = parse_size(&width.get(), &height.get()) {
                            document.update(|document| {
                                document.set_canvas_size(CanvasSize::Custom { width, height })
                            });
                        }
                    }
                }))
                .child(Text::new("Canvas Background").weight(700))
                .child(TextField::new(background.binding()).placeholder("#RRGGBBAA"))
                .child(
                    HStack::new()
                        .gap(StackGap::Small)
                        .child(icon_button::view("paint-bucket").on_click({
                            let document = document.clone();
                            let background = background.clone();
                            move || {
                                if let Some(color) = DocumentColor::from_hex(&background.get()) {
                                    document.update(|document| document.set_background(color));
                                }
                            }
                        }))
                        .child(icon_button::view("circle-off").on_click({
                            let document = document.clone();
                            move || {
                                document.update(|document| {
                                    document.set_background(DocumentColor::TRANSPARENT)
                                })
                            }
                        })),
                )
                .child(Divider::new())
                .child(icon_button::view("x").on_click(move || modal.close())),
        ),
    )
}

pub fn brush_settings(
    brushes: State<BrushLibrary>,
    preset_name: State<String>,
    status: State<String>,
    active_tool: State<EditorTool>,
    blob_width: State<f32>,
    modal: ModalState,
) -> impl View + 'static {
    let kind = if active_tool.get() == EditorTool::BlobBrush {
        BrushKind::Paint
    } else {
        BrushKind::Line
    };
    let active = brushes.get().active(kind).clone();
    let tip_description = match active.tip {
        BrushTip::Round => String::from("Round"),
        BrushTip::Ellipse { roundness, angle } => {
            format!("Ellipse — {:.0}% · {:.0}°", roundness * 100.0, angle)
        }
    };

    Card::new().content(
        Padding::all(16.0).content(
            VStack::new()
                .alignment(StackAlignment::Stretch)
                .gap(StackGap::Medium)
                .child(Text::new("Brush Settings").weight(700))
                .child(Text::new(active.name))
                .child(Divider::new())
                .child(Text::new("Tip").weight(700))
                .child(Text::new(tip_description))
                .child(
                    HStack::new()
                        .gap(StackGap::Small)
                        .child(brush_button("Round", brushes.clone(), kind, |brush| {
                            brush.tip = BrushTip::Round;
                        }))
                        .child(brush_button("Ellipse", brushes.clone(), kind, |brush| {
                            brush.tip = BrushTip::Ellipse {
                                roundness: 0.75,
                                angle: -45.0,
                            };
                        }))
                        .child(brush_button("Flatter", brushes.clone(), kind, |brush| {
                            if let BrushTip::Ellipse { roundness, .. } = &mut brush.tip {
                                *roundness -= 0.1;
                            }
                        }))
                        .child(brush_button("Rounder", brushes.clone(), kind, |brush| {
                            if let BrushTip::Ellipse { roundness, .. } = &mut brush.tip {
                                *roundness += 0.1;
                            }
                        }))
                        .child(brush_button("Rotate", brushes.clone(), kind, |brush| {
                            if let BrushTip::Ellipse { angle, .. } = &mut brush.tip {
                                *angle += 15.0;
                            }
                        })),
                )
                .child(Text::new(format!(
                    "Taper — {:.0}% / {:.0}%",
                    active.taper_start * 100.0,
                    active.taper_end * 100.0
                )))
                .child(
                    HStack::new()
                        .gap(StackGap::Small)
                        .child(brush_button("Less Taper", brushes.clone(), kind, |brush| {
                            brush.taper_start -= 0.1;
                            brush.taper_end -= 0.1;
                        }))
                        .child(brush_button("More Taper", brushes.clone(), kind, |brush| {
                            brush.taper_start += 0.1;
                            brush.taper_end += 0.1;
                        })),
                )
                .child(Divider::new())
                .child(Text::new("Preset").weight(700))
                .child(TextField::new(preset_name.binding()).placeholder("Preset Name"))
                .child(Text::new(status.get()))
                .child(
                    HStack::new()
                        .gap(StackGap::Small)
                        .child(icon_button::view("save").on_click({
                            let brushes = brushes.clone();
                            let preset_name = preset_name.clone();
                            let status = status.clone();
                            let active_tool = active_tool.clone();
                            let blob_width = blob_width.clone();
                            move || {
                                let result = brushes.update(|library| {
                                    if active_tool.get() == EditorTool::BlobBrush {
                                        library.update_active(kind, |brush| {
                                            brush.paint_width = blob_width.get()
                                        });
                                    }
                                    library.save_active_as_file(kind, &preset_name.get())
                                });
                                match result {
                                    Ok(path) => status.set(format!("Saved {}", path.display())),
                                    Err(error) => status.set(format!("Save failed: {error}")),
                                }
                            }
                        }))
                        .child(icon_button::view("refresh-cw").on_click({
                            let brushes = brushes.clone();
                            let status = status.clone();
                            let active_tool = active_tool.clone();
                            let blob_width = blob_width.clone();
                            move || match brushes.update(BrushLibrary::reload_from_disk) {
                                Ok(()) => {
                                    if active_tool.get() == EditorTool::BlobBrush {
                                        blob_width.set(
                                            brushes.get().active(BrushKind::Paint).paint_width,
                                        );
                                    }
                                    status.set(String::from("Brushes reloaded"));
                                }
                                Err(error) => status.set(format!("Reload failed: {error}")),
                            }
                        })),
                )
                .child(Divider::new())
                .child(icon_button::view("x").on_click(move || modal.close())),
        ),
    )
}

fn parse_size(width: &str, height: &str) -> Option<(f32, f32)> {
    let width = width.parse::<f32>().ok()?;
    let height = height.parse::<f32>().ok()?;
    (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
        .then_some((width, height))
}

fn brush_button(
    title: &'static str,
    brushes: State<BrushLibrary>,
    kind: BrushKind,
    update: impl Fn(&mut BrushDefinition) + 'static,
) -> Button {
    Button::new(title).on_click(move || {
        brushes.update(|library| library.update_active(kind, |brush| update(brush)));
    })
}
