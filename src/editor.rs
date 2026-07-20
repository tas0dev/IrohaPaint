#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    NodeEdit,
    Pen,
    Rectangle,
    Ellipse,
}

impl EditorTool {
    pub const ALL: [Self; 5] = [
        Self::Select,
        Self::NodeEdit,
        Self::Pen,
        Self::Rectangle,
        Self::Ellipse,
    ];
}
