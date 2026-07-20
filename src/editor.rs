#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    NodeEdit,
    Pencil,
    BlobBrush,
    Pen,
    Rectangle,
    Ellipse,
}

impl EditorTool {
    pub const ALL: [Self; 7] = [
        Self::Select,
        Self::NodeEdit,
        Self::Pencil,
        Self::BlobBrush,
        Self::Pen,
        Self::Rectangle,
        Self::Ellipse,
    ];
}
