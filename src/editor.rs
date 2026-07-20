#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    NodeEdit,
    Pencil,
    Paint,
    Fill,
    Eraser,
    BlobBrush,
    Pen,
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EraserMode {
    #[default]
    Partial,
    Object,
}

impl EraserMode {
    pub fn from_index(index: usize) -> Self {
        if index == 1 {
            Self::Object
        } else {
            Self::Partial
        }
    }
}

impl EditorTool {
    pub const ALL: [Self; 10] = [
        Self::Select,
        Self::NodeEdit,
        Self::Pencil,
        Self::Paint,
        Self::Fill,
        Self::Eraser,
        Self::BlobBrush,
        Self::Pen,
        Self::Rectangle,
        Self::Ellipse,
    ];
}
