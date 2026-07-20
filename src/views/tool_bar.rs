use viewkit::prelude::*;

use crate::editor::EditorTool;

pub fn view(active_tool: State<EditorTool>) -> impl View + 'static {
    let selected_tool = active_tool.get();
    let rows = EditorTool::ALL.into_iter().map(move |tool| {
        ListRow::new(tool_name(tool))
            .selected(selected_tool == tool)
            .on_select({
                let active_tool = active_tool.clone();

                move || active_tool.set(tool)
            })
    });

    Padding::all(8.0).content(
        VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::ExtraSmall)
            .children(rows),
    )
}

fn tool_name(tool: EditorTool) -> &'static str {
    match tool {
        EditorTool::Select => "Select",
        EditorTool::Pen => "Pen",
        EditorTool::Rectangle => "Rectangle",
        EditorTool::Ellipse => "Ellipse",
    }
}
