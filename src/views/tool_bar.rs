use viewkit::prelude::*;

use crate::brush::{BrushKind, BrushLibrary};
use crate::editor::EditorTool;

use super::icon_button;

pub fn view(active_tool: State<EditorTool>, pen_menu: PopupMenuState) -> impl View + 'static {
    let selected_tool = active_tool.get();
    let tools = EditorTool::ALL.into_iter().map(move |tool| {
        let button = icon_button::view(tool_icon(tool))
            .style(if selected_tool == tool {
                ButtonStyle::Standard
            } else {
                ButtonStyle::Ghost
            })
            .on_click({
                let active_tool = active_tool.clone();

                move || active_tool.set(tool)
            });
        if matches!(tool, EditorTool::Pencil | EditorTool::BlobBrush) {
            let active_tool_for_trigger = active_tool.clone();
            PopupMenuTrigger::new(button, pen_menu.clone())
                .when(move || active_tool_for_trigger.get() == tool)
                .into_stack_child()
        } else {
            button.into_stack_child()
        }
    });

    Padding::all(8.0).content(
        VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::ExtraSmall)
            .children(tools),
    )
}

pub fn pen_menu(
    brushes: State<BrushLibrary>,
    brush_settings: ModalState,
    stroke_color: State<Color>,
    brush_width: State<f32>,
    blob_width: State<f32>,
    smoothing: State<f32>,
    active_tool: State<EditorTool>,
) -> Menu {
    let library = brushes.get();
    let kind = if active_tool.get() == EditorTool::BlobBrush {
        BrushKind::Paint
    } else {
        BrushKind::Line
    };
    let active = library.active_index(kind);
    let mut menu = Menu::new();
    for (index, brush) in library
        .presets()
        .iter()
        .enumerate()
        .filter(|(_, brush)| brush.kind == kind)
    {
        let label = if index == active {
            format!("✓ {}", brush.name)
        } else {
            brush.name.clone()
        };
        menu = menu.item(MenuItem::new(label).on_select({
            let brushes = brushes.clone();
            let stroke_color = stroke_color.clone();
            let brush_width = brush_width.clone();
            let blob_width = blob_width.clone();
            let smoothing = smoothing.clone();
            let active_tool = active_tool.clone();
            move || {
                brushes.update(|library| library.select(kind, index));
                let brush = brushes.get().active(kind).clone();
                stroke_color.set(Color::rgba(
                    brush.color.red,
                    brush.color.green,
                    brush.color.blue,
                    brush.color.alpha,
                ));
                if active_tool.get() == EditorTool::BlobBrush {
                    blob_width.set(brush.paint_width);
                } else {
                    brush_width.set(brush.width);
                }
                smoothing.set(brush.smoothing);
            }
        }));
    }
    menu.separator()
        .item(MenuItem::new("Edit Brush…").on_select(move || {
            brush_settings.open();
        }))
}

fn tool_icon(tool: EditorTool) -> &'static str {
    match tool {
        EditorTool::Select => "mouse-pointer-2",
        EditorTool::NodeEdit => "spline",
        EditorTool::Pencil => "pen-tool",
        EditorTool::Paint => "brush",
        EditorTool::Fill => "paint-bucket",
        EditorTool::Eraser => "eraser",
        EditorTool::BlobBrush => "droplet",
        EditorTool::Pen => "line",
        EditorTool::Rectangle => "square",
        EditorTool::Ellipse => "circle",
    }
}
