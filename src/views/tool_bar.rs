use viewkit::prelude::*;

use crate::editor::EditorTool;

use super::icon_button;

pub fn view(active_tool: State<EditorTool>) -> impl View + 'static {
    let selected_tool = active_tool.get();
    let tools = EditorTool::ALL.into_iter().map(move |tool| {
        icon_button::view(tool_icon(tool))
            .style(if selected_tool == tool {
                ButtonStyle::Standard
            } else {
                ButtonStyle::Ghost
            })
            .on_click({
                let active_tool = active_tool.clone();

                move || active_tool.set(tool)
            })
    });

    Padding::all(8.0).content(
        VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::ExtraSmall)
            .children(tools),
    )
}

fn tool_icon(tool: EditorTool) -> &'static str {
    match tool {
        EditorTool::Select => "mouse-pointer-2",
        EditorTool::NodeEdit => "spline",
        EditorTool::Pen => "pen-tool",
        EditorTool::Rectangle => "square",
        EditorTool::Ellipse => "circle",
    }
}
