#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    NodeEdit,
    Pencil,
    Pen,
    Rectangle,
    Ellipse,
}

impl EditorTool {
    pub const ALL: [Self; 6] = [
        Self::Select,
        Self::NodeEdit,
        Self::Pencil,
        Self::Pen,
        Self::Rectangle,
        Self::Ellipse,
    ];
}
