#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    NodeEdit,
    Pencil,
    Paint,
    BlobBrush,
    Pen,
    Rectangle,
    Ellipse,
}

impl EditorTool {
    pub const ALL: [Self; 8] = [
        Self::Select,
        Self::NodeEdit,
        Self::Pencil,
        Self::Paint,
        Self::BlobBrush,
        Self::Pen,
        Self::Rectangle,
        Self::Ellipse,
    ];
}
