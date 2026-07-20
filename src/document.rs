#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentPoint {
    pub x: f32,
    pub y: f32,
}

impl DocumentPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl DocumentRect {
    pub fn from_points(first: DocumentPoint, second: DocumentPoint) -> Self {
        Self {
            x: first.x.min(second.x),
            y: first.y.min(second.y),
            width: (second.x - first.x).abs(),
            height: (second.y - first.y).abs(),
        }
    }

    pub fn contains(self, point: DocumentPoint) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub fn translated(self, delta: DocumentPoint) -> Self {
        Self {
            x: self.x + delta.x,
            y: self.y + delta.y,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(u64);

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    Rectangle { bounds: DocumentRect },
    Ellipse { bounds: DocumentRect },
    Path { points: Vec<DocumentPoint> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentObject {
    id: ObjectId,
    kind: ObjectKind,
}

impl DocumentObject {
    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn kind(&self) -> &ObjectKind {
        &self.kind
    }

    pub fn bounds(&self) -> DocumentRect {
        match &self.kind {
            ObjectKind::Rectangle { bounds } | ObjectKind::Ellipse { bounds } => *bounds,
            ObjectKind::Path { points } => path_bounds(points),
        }
    }

    fn set_bounds(&mut self, new_bounds: DocumentRect) {
        match &mut self.kind {
            ObjectKind::Rectangle { bounds } | ObjectKind::Ellipse { bounds } => {
                *bounds = new_bounds;
            }
            ObjectKind::Path { points } => {
                let old_bounds = path_bounds(points);
                for point in points {
                    point.x = scale_axis(
                        point.x,
                        old_bounds.x,
                        old_bounds.width,
                        new_bounds.x,
                        new_bounds.width,
                    );
                    point.y = scale_axis(
                        point.y,
                        old_bounds.y,
                        old_bounds.height,
                        new_bounds.y,
                        new_bounds.height,
                    );
                }
            }
        }
    }

    fn translate(&mut self, delta: DocumentPoint) {
        match &mut self.kind {
            ObjectKind::Rectangle { bounds } | ObjectKind::Ellipse { bounds } => {
                *bounds = bounds.translated(delta);
            }
            ObjectKind::Path { points } => {
                for point in points {
                    point.x += delta.x;
                    point.y += delta.y;
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    name: String,
    objects: Vec<DocumentObject>,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            objects: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn objects(&self) -> &[DocumentObject] {
        &self.objects
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DocumentSnapshot {
    layers: Vec<Layer>,
    selected_layer: Option<usize>,
    selected_object: Option<ObjectId>,
    next_object_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    layers: Vec<Layer>,
    selected_layer: Option<usize>,
    selected_object: Option<ObjectId>,
    next_object_id: u64,
    undo_stack: Vec<DocumentSnapshot>,
    redo_stack: Vec<DocumentSnapshot>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            layers: vec![Layer::new("Layer 1")],
            selected_layer: Some(0),
            selected_object: None,
            next_object_id: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn selected_layer(&self) -> Option<usize> {
        self.selected_layer
    }

    pub fn select_layer(&mut self, index: usize) {
        if index < self.layers.len() {
            self.selected_layer = Some(index);
            self.selected_object = None;
        }
    }

    pub fn selected_object(&self) -> Option<ObjectId> {
        self.selected_object
    }

    pub fn select_object(&mut self, id: Option<ObjectId>) {
        self.selected_object = id.filter(|id| self.object(*id).is_some());
    }

    pub fn object(&self, id: ObjectId) -> Option<&DocumentObject> {
        self.layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
            .find(|object| object.id == id)
    }

    pub fn add_rectangle(&mut self, bounds: DocumentRect) -> ObjectId {
        self.add_object(ObjectKind::Rectangle { bounds })
    }

    pub fn add_ellipse(&mut self, bounds: DocumentRect) -> ObjectId {
        self.add_object(ObjectKind::Ellipse { bounds })
    }

    pub fn add_path(&mut self, points: Vec<DocumentPoint>) -> ObjectId {
        self.add_object(ObjectKind::Path { points })
    }

    pub fn resize_object(&mut self, id: ObjectId, bounds: DocumentRect) {
        self.edit_object(id, |object| object.set_bounds(bounds));
    }

    pub fn translate_object(&mut self, id: ObjectId, delta: DocumentPoint) {
        self.edit_object(id, |object| object.translate(delta));
    }

    pub fn delete_selected_object(&mut self) {
        let Some(id) = self.selected_object else {
            return;
        };
        let Some((layer_index, object_index)) = self.find_object_index(id) else {
            return;
        };

        self.record_change();
        self.layers[layer_index].objects.remove(object_index);
        self.selected_object = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = self.snapshot();
            self.redo_stack.push(current);
            self.restore(snapshot);
        }
    }

    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = self.snapshot();
            self.undo_stack.push(current);
            self.restore(snapshot);
        }
    }

    fn add_object(&mut self, kind: ObjectKind) -> ObjectId {
        self.record_change();

        let id = ObjectId(self.next_object_id);
        self.next_object_id += 1;
        let object = DocumentObject { id, kind };

        let layer_index = self.selected_layer.unwrap_or(0);
        self.layers[layer_index].objects.push(object);
        self.selected_object = Some(id);
        id
    }

    fn edit_object(&mut self, id: ObjectId, edit: impl FnOnce(&mut DocumentObject)) {
        let Some((layer_index, object_index)) = self.find_object_index(id) else {
            return;
        };

        self.record_change();
        edit(&mut self.layers[layer_index].objects[object_index]);
        self.selected_object = Some(id);
    }

    fn find_object_index(&self, id: ObjectId) -> Option<(usize, usize)> {
        self.layers
            .iter()
            .enumerate()
            .find_map(|(layer_index, layer)| {
                layer
                    .objects
                    .iter()
                    .position(|object| object.id == id)
                    .map(|object_index| (layer_index, object_index))
            })
    }

    fn record_change(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            layers: self.layers.clone(),
            selected_layer: self.selected_layer,
            selected_object: self.selected_object,
            next_object_id: self.next_object_id,
        }
    }

    fn restore(&mut self, snapshot: DocumentSnapshot) {
        self.layers = snapshot.layers;
        self.selected_layer = snapshot.selected_layer;
        self.selected_object = snapshot.selected_object;
        self.next_object_id = snapshot.next_object_id;
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

fn path_bounds(points: &[DocumentPoint]) -> DocumentRect {
    let Some(first) = points.first() else {
        return DocumentRect::default();
    };

    let (mut min_x, mut max_x) = (first.x, first.x);
    let (mut min_y, mut max_y) = (first.y, first.y);
    for point in &points[1..] {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }

    DocumentRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

fn scale_axis(value: f32, old_start: f32, old_size: f32, new_start: f32, new_size: f32) -> f32 {
    if old_size.abs() <= f32::EPSILON {
        new_start
    } else {
        new_start + (value - old_start) / old_size * new_size
    }
}
