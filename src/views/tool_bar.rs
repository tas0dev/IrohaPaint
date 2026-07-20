use viewkit::prelude::*;

use crate::brush::BrushLibrary;
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
        if tool == EditorTool::Pencil {
            let active_tool_for_trigger = active_tool.clone();
            PopupMenuTrigger::new(button, pen_menu.clone())
                .when(move || active_tool_for_trigger.get() == EditorTool::Pencil)
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

pub fn pen_menu(brushes: State<BrushLibrary>, brush_settings: ModalState) -> Menu {
    let library = brushes.get();
    let active = library.active_index();
    let mut menu = Menu::new();
    for (index, brush) in library.presets().iter().enumerate() {
        let label = if index == active {
            format!("✓ {}", brush.name)
        } else {
            brush.name.clone()
        };
        menu = menu.item(MenuItem::new(label).on_select({
            let brushes = brushes.clone();
            move || brushes.update(|library| library.select(index))
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
        EditorTool::Pen => "line",
        EditorTool::Rectangle => "square",
        EditorTool::Ellipse => "circle",
    }
}
