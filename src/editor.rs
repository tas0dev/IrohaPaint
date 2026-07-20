#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    Pen,
    Rectangle,
    Ellipse,
}

impl EditorTool {
    pub const ALL: [Self; 4] = [Self::Select, Self::Pen, Self::Rectangle, Self::Ellipse];
}
