mod paint_layer;
mod path_edit;
mod path_erase;
mod stroke_outline;

const FILL_BLEED_WIDTH: f32 = 2.0;

pub(crate) use paint_layer::{PAINT_TILE_SIZE, PaintDab, PaintLayer};
use path_edit::simplification_candidates;
use path_erase::{erase_path, eraser_cutout};
pub(crate) use stroke_outline::variable_stroke_outlines;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl DocumentColor {
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub fn from_hex(value: &str) -> Option<Self> {
        let value = value.trim().trim_start_matches('#');
        let number = u32::from_str_radix(value, 16).ok()?;
        match value.len() {
            6 => Some(Self::rgba(
                ((number >> 16) & 0xff) as u8,
                ((number >> 8) & 0xff) as u8,
                (number & 0xff) as u8,
                255,
            )),
            8 => Some(Self::rgba(
                ((number >> 24) & 0xff) as u8,
                ((number >> 16) & 0xff) as u8,
                ((number >> 8) & 0xff) as u8,
                (number & 0xff) as u8,
            )),
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            self.red, self.green, self.blue, self.alpha
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasSize {
    FitArtwork,
    Custom { width: f32, height: f32 },
}

impl Default for CanvasSize {
    fn default() -> Self {
        Self::Custom {
            width: 1200.0,
            height: 1200.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentProperties {
    pub canvas_size: CanvasSize,
    pub background: DocumentColor,
}

impl Default for DocumentColor {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierNode {
    pub position: DocumentPoint,
    pub handle_in: DocumentPoint,
    pub handle_out: DocumentPoint,
    pub kind: NodeKind,
    /// Relative width of a variable-width stroke at this node.
    pub width: f32,
}

impl BezierNode {
    pub const fn corner(position: DocumentPoint) -> Self {
        Self {
            position,
            handle_in: position,
            handle_out: position,
            kind: NodeKind::Corner,
            width: 1.0,
        }
    }

    pub fn smooth(position: DocumentPoint, handle_out: DocumentPoint) -> Self {
        Self {
            position,
            handle_in: mirror_point(handle_out, position),
            handle_out,
            kind: NodeKind::Symmetric,
            width: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeKind {
    #[default]
    Corner,
    Smooth,
    Symmetric,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BezierPath {
    nodes: Vec<BezierNode>,
    closed: bool,
}

impl BezierPath {
    pub fn new(first_node: BezierNode) -> Self {
        Self {
            nodes: vec![first_node],
            closed: false,
        }
    }

    pub fn nodes(&self) -> &[BezierNode] {
        &self.nodes
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub(crate) fn from_nodes(nodes: Vec<BezierNode>) -> Option<Self> {
        if nodes.len() < 2 {
            return None;
        }
        Some(Self {
            nodes,
            closed: false,
        })
    }

    pub(crate) fn push_node(&mut self, node: BezierNode) {
        self.nodes.push(node);
    }

    pub(crate) fn close(&mut self) {
        if self.nodes.len() > 1 {
            self.closed = true;
        }
    }

    pub(crate) fn edit_node(
        &mut self,
        node_index: usize,
        component: NodeComponent,
        point: DocumentPoint,
        independent: bool,
    ) {
        let Some(node) = self.nodes.get_mut(node_index) else {
            return;
        };
        match component {
            NodeComponent::Anchor => {
                let delta =
                    DocumentPoint::new(point.x - node.position.x, point.y - node.position.y);
                node.position = point;
                translate_point(&mut node.handle_in, delta);
                translate_point(&mut node.handle_out, delta);
            }
            NodeComponent::HandleIn => {
                node.handle_in = point;
                constrain_opposite_handle(node, NodeComponent::HandleIn, independent);
            }
            NodeComponent::HandleOut => {
                node.handle_out = point;
                constrain_opposite_handle(node, NodeComponent::HandleOut, independent);
            }
        }
    }

    pub(crate) fn translate_nodes(&mut self, indices: &[usize], delta: DocumentPoint) {
        for (index, node) in self.nodes.iter_mut().enumerate() {
            if indices.contains(&index) {
                translate_point(&mut node.position, delta);
                translate_point(&mut node.handle_in, delta);
                translate_point(&mut node.handle_out, delta);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeComponent {
    Anchor,
    HandleIn,
    HandleOut,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrokeCap {
    Butt,
    #[default]
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrokeJoin {
    Miter,
    #[default]
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub minimum_width: f32,
    pub taper_start: f32,
    pub taper_end: f32,
    pub tip_roundness: f32,
    pub tip_angle: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub color: DocumentColor,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 2.0,
            minimum_width: 1.0,
            taper_start: 0.0,
            taper_end: 0.0,
            tip_roundness: 1.0,
            tip_angle: 0.0,
            cap: StrokeCap::Round,
            join: StrokeJoin::Round,
            color: DocumentColor::BLACK,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObjectStyle {
    pub stroke: StrokeStyle,
    pub fill: DocumentColor,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    Rectangle {
        bounds: DocumentRect,
        style: ObjectStyle,
    },
    Ellipse {
        bounds: DocumentRect,
        style: ObjectStyle,
    },
    Path {
        path: BezierPath,
        style: ObjectStyle,
        variable_width: bool,
        cutouts: Vec<BezierPath>,
    },
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

    pub fn style(&self) -> ObjectStyle {
        match self.kind {
            ObjectKind::Rectangle { style, .. }
            | ObjectKind::Ellipse { style, .. }
            | ObjectKind::Path { style, .. } => style,
        }
    }

    pub fn bounds(&self) -> DocumentRect {
        match &self.kind {
            ObjectKind::Rectangle { bounds, .. } | ObjectKind::Ellipse { bounds, .. } => *bounds,
            ObjectKind::Path { path, .. } => path_bounds(path),
        }
    }

    fn set_bounds(&mut self, new_bounds: DocumentRect) {
        match &mut self.kind {
            ObjectKind::Rectangle { bounds, .. } | ObjectKind::Ellipse { bounds, .. } => {
                *bounds = new_bounds;
            }
            ObjectKind::Path { path, cutouts, .. } => {
                let old_bounds = path_bounds(path);
                for node in &mut path.nodes {
                    scale_point(&mut node.position, old_bounds, new_bounds);
                    scale_point(&mut node.handle_in, old_bounds, new_bounds);
                    scale_point(&mut node.handle_out, old_bounds, new_bounds);
                }
                for cutout in cutouts {
                    for node in &mut cutout.nodes {
                        scale_point(&mut node.position, old_bounds, new_bounds);
                        scale_point(&mut node.handle_in, old_bounds, new_bounds);
                        scale_point(&mut node.handle_out, old_bounds, new_bounds);
                    }
                }
            }
        }
    }

    fn translate(&mut self, delta: DocumentPoint) {
        match &mut self.kind {
            ObjectKind::Rectangle { bounds, .. } | ObjectKind::Ellipse { bounds, .. } => {
                *bounds = bounds.translated(delta);
            }
            ObjectKind::Path { path, cutouts, .. } => {
                for node in &mut path.nodes {
                    translate_point(&mut node.position, delta);
                    translate_point(&mut node.handle_in, delta);
                    translate_point(&mut node.handle_out, delta);
                }
                for cutout in cutouts {
                    for node in &mut cutout.nodes {
                        translate_point(&mut node.position, delta);
                        translate_point(&mut node.handle_in, delta);
                        translate_point(&mut node.handle_out, delta);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    name: String,
    objects: Vec<DocumentObject>,
    paint: PaintLayer,
    visible: bool,
    clipped: bool,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            objects: Vec::new(),
            paint: PaintLayer::default(),
            visible: true,
            clipped: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn objects(&self) -> &[DocumentObject] {
        &self.objects
    }

    pub fn paint(&self) -> &PaintLayer {
        &self.paint
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_clipped(&self) -> bool {
        self.clipped
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DocumentSnapshot {
    layers: Vec<Layer>,
    selected_layer: Option<usize>,
    selected_object: Option<ObjectId>,
    next_object_id: u64,
    properties: DocumentProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    layers: Vec<Layer>,
    selected_layer: Option<usize>,
    selected_object: Option<ObjectId>,
    next_object_id: u64,
    undo_stack: Vec<DocumentSnapshot>,
    redo_stack: Vec<DocumentSnapshot>,
    properties: DocumentProperties,
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
            properties: DocumentProperties::default(),
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn properties(&self) -> DocumentProperties {
        self.properties
    }

    pub fn set_canvas_size(&mut self, canvas_size: CanvasSize) {
        let canvas_size = match canvas_size {
            CanvasSize::Custom { width, height }
                if width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite() =>
            {
                CanvasSize::Custom { width, height }
            }
            CanvasSize::Custom { .. } => return,
            CanvasSize::FitArtwork => CanvasSize::FitArtwork,
        };
        if self.properties.canvas_size != canvas_size {
            self.record_change();
            self.properties.canvas_size = canvas_size;
        }
    }

    pub fn set_background(&mut self, background: DocumentColor) {
        if self.properties.background != background {
            self.record_change();
            self.properties.background = background;
        }
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

    pub fn add_layer(&mut self) -> usize {
        self.record_change();
        let mut number = self.layers.len() + 1;
        let name = loop {
            let candidate = format!("Layer {number}");
            if self.layers.iter().all(|layer| layer.name != candidate) {
                break candidate;
            }
            number += 1;
        };
        // A new layer belongs directly above the layer the artist is working on.
        // Besides matching the visible layer stack, this makes "add, then clip"
        // reliably target the intended base layer instead of an unrelated top layer.
        let index = self
            .selected_layer
            .map_or(self.layers.len(), |selected| selected + 1);
        self.layers.insert(index, Layer::new(name));
        self.selected_layer = Some(index);
        self.selected_object = None;
        index
    }

    pub fn rename_selected_layer(&mut self, name: &str) -> bool {
        let Some(index) = self.selected_layer else {
            return false;
        };
        let name = name.trim();
        if name.is_empty() || self.layers[index].name == name {
            return false;
        }
        self.record_change();
        self.layers[index].name = name.to_owned();
        true
    }

    pub fn delete_selected_layer(&mut self) -> bool {
        if self.layers.len() <= 1 {
            return false;
        }
        let Some(index) = self.selected_layer else {
            return false;
        };
        self.record_change();
        self.layers.remove(index);
        self.layers[0].clipped = false;
        self.selected_layer = Some(index.min(self.layers.len() - 1));
        self.selected_object = None;
        true
    }

    pub fn toggle_selected_layer_visibility(&mut self) -> bool {
        let Some(index) = self.selected_layer else {
            return false;
        };
        self.toggle_layer_visibility(index)
    }

    pub fn toggle_layer_visibility(&mut self, index: usize) -> bool {
        if index >= self.layers.len() {
            return false;
        }
        self.record_change();
        self.layers[index].visible = !self.layers[index].visible;
        self.selected_object = None;
        true
    }

    pub fn toggle_selected_layer_clipping(&mut self) -> bool {
        let Some(index) = self.selected_layer else {
            return false;
        };
        self.toggle_layer_clipping(index)
    }

    pub fn toggle_layer_clipping(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.layers.len() {
            return false;
        }
        self.record_change();
        self.layers[index].clipped = !self.layers[index].clipped;
        self.selected_object = None;
        true
    }

    /// Returns the shared base for a clipping stack.
    ///
    /// Consecutive clipped layers all use the first non-clipped layer below
    /// them, which is the behavior expected when stacking shadow and highlight
    /// layers over one anime-color base.
    pub fn clip_base_layer(&self, layer_index: usize) -> Option<usize> {
        (layer_index < self.layers.len() && self.layers[layer_index].clipped)
            .then(|| {
                (0..layer_index)
                    .rev()
                    .find(|index| !self.layers[*index].clipped)
            })
            .flatten()
    }

    pub fn move_selected_layer_up(&mut self) -> bool {
        let Some(index) = self.selected_layer else {
            return false;
        };
        if index + 1 >= self.layers.len() {
            return false;
        }
        self.record_change();
        self.layers.swap(index, index + 1);
        self.layers[0].clipped = false;
        self.selected_layer = Some(index + 1);
        self.selected_object = None;
        true
    }

    pub fn move_selected_layer_down(&mut self) -> bool {
        let Some(index) = self.selected_layer else {
            return false;
        };
        if index == 0 {
            return false;
        }
        self.record_change();
        self.layers.swap(index, index - 1);
        self.layers[0].clipped = false;
        self.selected_layer = Some(index - 1);
        self.selected_object = None;
        true
    }

    pub fn selected_object(&self) -> Option<ObjectId> {
        self.selected_object
    }

    pub fn select_object(&mut self, id: Option<ObjectId>) {
        let selected_layer = self.selected_layer;
        self.selected_object = id.filter(|id| {
            selected_layer.is_some_and(|layer_index| {
                self.layers[layer_index]
                    .objects
                    .iter()
                    .any(|object| object.id == *id)
            })
        });
    }

    pub fn begin_paint_stroke(&mut self, dabs: &[PaintDab]) -> bool {
        let selected_visible = self
            .selected_layer
            .and_then(|index| self.layers.get(index))
            .is_some_and(|layer| layer.visible);
        if dabs.is_empty() || !selected_visible {
            return false;
        }
        self.record_change();
        self.selected_object = None;
        self.apply_paint_dabs(dabs);
        true
    }

    pub fn continue_paint_stroke(&mut self, dabs: &[PaintDab]) {
        if !dabs.is_empty() {
            self.apply_paint_dabs(dabs);
        }
    }

    pub fn cancel_in_progress_change(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.restore(snapshot);
            self.redo_stack.clear();
        }
    }

    pub fn fill_from_outline(
        &mut self,
        source_layer: usize,
        id: ObjectId,
        color: DocumentColor,
    ) -> bool {
        let Some(target_layer) = self.selected_layer else {
            return false;
        };
        if !self.layers[target_layer].visible || !self.layers[source_layer].visible {
            return false;
        }
        let Some(object_index) = self.layers[source_layer]
            .objects
            .iter()
            .position(|object| object.id == id)
        else {
            return false;
        };
        if source_layer == target_layer {
            let current = self.layers[source_layer].objects[object_index].style().fill;
            if current == color {
                return false;
            }
            self.record_change();
            match &mut self.layers[source_layer].objects[object_index].kind {
                ObjectKind::Rectangle { style, .. }
                | ObjectKind::Ellipse { style, .. }
                | ObjectKind::Path { style, .. } => style.fill = color,
            }
            self.selected_object = Some(id);
            return true;
        }
        let source = self.layers[source_layer].objects[object_index].kind.clone();
        let style = ObjectStyle {
            stroke: StrokeStyle {
                color,
                width: FILL_BLEED_WIDTH,
                ..StrokeStyle::default()
            },
            fill: color,
        };
        let kind = match source {
            ObjectKind::Rectangle { bounds, .. } => ObjectKind::Rectangle { bounds, style },
            ObjectKind::Ellipse { bounds, .. } => ObjectKind::Ellipse { bounds, style },
            ObjectKind::Path { path, cutouts, .. } if path.is_closed() => ObjectKind::Path {
                path,
                style,
                variable_width: false,
                cutouts,
            },
            ObjectKind::Path { .. } => return false,
        };
        self.add_object(kind);
        true
    }

    pub fn begin_erasing_objects(&mut self, ids: &[ObjectId]) -> bool {
        if !self.can_erase_objects(ids) {
            return false;
        }
        self.record_change();
        self.erase_objects(ids)
    }

    pub fn continue_erasing_objects(&mut self, ids: &[ObjectId]) -> bool {
        self.erase_objects(ids)
    }

    pub fn begin_erasing_path_sections(&mut self, centers: &[DocumentPoint], radius: f32) -> bool {
        let snapshot = self.snapshot();
        if !self.erase_path_sections(centers, radius) {
            return false;
        }
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
        true
    }

    pub fn continue_erasing_path_sections(
        &mut self,
        centers: &[DocumentPoint],
        radius: f32,
    ) -> bool {
        self.erase_path_sections(centers, radius)
    }

    pub fn object(&self, id: ObjectId) -> Option<&DocumentObject> {
        self.layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
            .find(|object| object.id == id)
    }

    pub fn add_rectangle(&mut self, bounds: DocumentRect) -> ObjectId {
        self.add_rectangle_with_style(bounds, ObjectStyle::default())
    }

    pub fn add_rectangle_with_style(
        &mut self,
        bounds: DocumentRect,
        style: ObjectStyle,
    ) -> ObjectId {
        self.add_object(ObjectKind::Rectangle { bounds, style })
    }

    pub fn add_ellipse(&mut self, bounds: DocumentRect) -> ObjectId {
        self.add_ellipse_with_style(bounds, ObjectStyle::default())
    }

    pub fn add_ellipse_with_style(&mut self, bounds: DocumentRect, style: ObjectStyle) -> ObjectId {
        self.add_object(ObjectKind::Ellipse { bounds, style })
    }

    pub fn add_path(&mut self, first_node: BezierNode) -> ObjectId {
        self.add_path_with_style(first_node, StrokeStyle::default())
    }

    pub fn add_path_with_style(&mut self, first_node: BezierNode, stroke: StrokeStyle) -> ObjectId {
        self.add_path_with_object_style(
            first_node,
            ObjectStyle {
                stroke,
                ..ObjectStyle::default()
            },
        )
    }

    pub fn add_path_with_object_style(
        &mut self,
        first_node: BezierNode,
        style: ObjectStyle,
    ) -> ObjectId {
        self.add_object(ObjectKind::Path {
            path: BezierPath::new(first_node),
            style,
            variable_width: false,
            cutouts: Vec::new(),
        })
    }

    pub fn add_variable_width_path(&mut self, path: BezierPath, stroke: StrokeStyle) -> ObjectId {
        self.add_object(ObjectKind::Path {
            path,
            style: ObjectStyle {
                stroke,
                ..ObjectStyle::default()
            },
            variable_width: true,
            cutouts: Vec::new(),
        })
    }

    pub fn add_styled_path(&mut self, path: BezierPath, style: ObjectStyle) -> ObjectId {
        self.add_object(ObjectKind::Path {
            path,
            style,
            variable_width: false,
            cutouts: Vec::new(),
        })
    }

    pub fn add_fill_region(&mut self, path: BezierPath, color: DocumentColor) -> ObjectId {
        self.add_styled_path(
            path,
            ObjectStyle {
                stroke: StrokeStyle {
                    color,
                    width: FILL_BLEED_WIDTH,
                    ..StrokeStyle::default()
                },
                fill: color,
            },
        )
    }

    pub fn append_path_node(&mut self, id: ObjectId, node: BezierNode) {
        self.edit_object(id, |object| {
            if let ObjectKind::Path { path, .. } = &mut object.kind
                && !path.closed
            {
                path.nodes.push(node);
            }
        });
    }

    pub fn insert_path_node(&mut self, id: ObjectId, start_index: usize, t: f32) -> Option<usize> {
        let (layer_index, object_index) = self.find_object_index(id)?;
        let ObjectKind::Path { path, .. } = &self.layers[layer_index].objects[object_index].kind
        else {
            return None;
        };
        if path.nodes.len() < 2 {
            return None;
        }
        self.record_change();
        let ObjectKind::Path { path, .. } =
            &mut self.layers[layer_index].objects[object_index].kind
        else {
            unreachable!();
        };
        let inserted = path.insert_node_on_segment(start_index, t);
        if inserted.is_some() {
            self.selected_object = Some(id);
        }
        inserted
    }

    pub fn set_path_node_kinds(&mut self, id: ObjectId, indices: &[usize], kind: NodeKind) {
        self.edit_object(id, |object| {
            if let ObjectKind::Path { path, .. } = &mut object.kind {
                path.set_node_kinds(indices, kind);
            }
        });
    }

    pub fn smooth_path_nodes(&mut self, id: ObjectId, indices: &[usize]) {
        self.edit_object(id, |object| {
            if let ObjectKind::Path { path, .. } = &mut object.kind {
                path.smooth_nodes(indices);
            }
        });
    }

    pub fn simplify_path_nodes(&mut self, id: ObjectId, indices: &[usize], tolerance: f32) -> bool {
        let Some(object) = self.object(id) else {
            return false;
        };
        let ObjectKind::Path { path, .. } = object.kind() else {
            return false;
        };
        let removable = simplification_candidates(path, indices, tolerance);
        if removable.is_empty() {
            return false;
        }
        self.edit_object(id, |object| {
            if let ObjectKind::Path { path, .. } = &mut object.kind {
                path.remove_nodes_preserving_shape(&removable);
            }
        });
        true
    }

    pub fn close_path(&mut self, id: ObjectId) {
        self.edit_object(id, |object| {
            if let ObjectKind::Path { path, .. } = &mut object.kind
                && path.nodes.len() > 1
            {
                path.closed = true;
            }
        });
    }

    pub fn edit_path_node(
        &mut self,
        id: ObjectId,
        node_index: usize,
        component: NodeComponent,
        point: DocumentPoint,
        independent: bool,
    ) {
        self.edit_object(id, |object| {
            let ObjectKind::Path { path, .. } = &mut object.kind else {
                return;
            };
            path.edit_node(node_index, component, point, independent);
        });
    }

    pub fn translate_path_nodes(
        &mut self,
        id: ObjectId,
        node_indices: &[usize],
        delta: DocumentPoint,
    ) {
        self.edit_object(id, |object| {
            let ObjectKind::Path { path, .. } = &mut object.kind else {
                return;
            };
            path.translate_nodes(node_indices, delta);
        });
    }

    pub fn remove_path_node(&mut self, id: ObjectId, node_index: usize) {
        self.remove_path_nodes(id, &[node_index]);
    }

    pub fn remove_path_nodes(&mut self, id: ObjectId, node_indices: &[usize]) {
        let Some((layer_index, object_index)) = self.find_object_index(id) else {
            return;
        };
        let ObjectKind::Path { path, .. } = &self.layers[layer_index].objects[object_index].kind
        else {
            return;
        };
        if node_indices.is_empty() || !node_indices.iter().any(|index| *index < path.nodes.len()) {
            return;
        }

        self.record_change();
        let ObjectKind::Path { path, .. } =
            &mut self.layers[layer_index].objects[object_index].kind
        else {
            unreachable!();
        };
        path.remove_nodes_preserving_shape(node_indices);
        if path.nodes.is_empty() {
            self.layers[layer_index].objects.remove(object_index);
            self.selected_object = None;
        } else if path.nodes.len() < 2 {
            path.closed = false;
        }
    }

    pub fn resize_object(&mut self, id: ObjectId, bounds: DocumentRect) {
        self.edit_object(id, |object| object.set_bounds(bounds));
    }

    pub fn translate_object(&mut self, id: ObjectId, delta: DocumentPoint) {
        self.edit_object(id, |object| object.translate(delta));
    }

    pub fn set_selected_stroke_color(&mut self, color: DocumentColor) {
        let Some(id) = self.selected_object else {
            return;
        };
        self.edit_object(id, |object| match &mut object.kind {
            ObjectKind::Rectangle { style, .. }
            | ObjectKind::Ellipse { style, .. }
            | ObjectKind::Path { style, .. } => style.stroke.color = color,
        });
    }

    pub fn set_selected_stroke_width(&mut self, width: f32) {
        let Some(id) = self.selected_object else {
            return;
        };
        self.edit_object(id, |object| match &mut object.kind {
            ObjectKind::Rectangle { style, .. }
            | ObjectKind::Ellipse { style, .. }
            | ObjectKind::Path { style, .. } => style.stroke.width = width.max(0.1),
        });
    }

    pub fn set_selected_fill_color(&mut self, color: DocumentColor) {
        let Some(id) = self.selected_object else {
            return;
        };
        self.edit_object(id, |object| match &mut object.kind {
            ObjectKind::Rectangle { style, .. }
            | ObjectKind::Ellipse { style, .. }
            | ObjectKind::Path { style, .. } => style.fill = color,
        });
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

    fn can_erase_objects(&self, ids: &[ObjectId]) -> bool {
        self.selected_layer
            .and_then(|index| self.layers.get(index))
            .filter(|layer| layer.visible)
            .is_some_and(|layer| layer.objects.iter().any(|object| ids.contains(&object.id)))
    }

    fn erase_objects(&mut self, ids: &[ObjectId]) -> bool {
        let Some(layer_index) = self.selected_layer else {
            return false;
        };
        if !self.layers[layer_index].visible {
            return false;
        }
        let old_len = self.layers[layer_index].objects.len();
        self.layers[layer_index]
            .objects
            .retain(|object| !ids.contains(&object.id));
        let changed = self.layers[layer_index].objects.len() != old_len;
        if changed && self.selected_object.is_some_and(|id| ids.contains(&id)) {
            self.selected_object = None;
        }
        changed
    }

    fn erase_path_sections(&mut self, centers: &[DocumentPoint], radius: f32) -> bool {
        let Some(layer_index) = self.selected_layer else {
            return false;
        };
        if !self.layers[layer_index].visible || centers.is_empty() || radius <= 0.0 {
            return false;
        }

        let old_objects = std::mem::take(&mut self.layers[layer_index].objects);
        let mut objects = Vec::with_capacity(old_objects.len());
        let mut changed = false;
        for object in old_objects {
            let DocumentObject { id, kind } = object;
            match kind {
                ObjectKind::Path {
                    path,
                    style,
                    variable_width,
                    mut cutouts,
                } => {
                    if path.is_closed() && style.fill.alpha > 0 {
                        if let Some(cutout) = eraser_cutout(centers, radius)
                            && rects_intersect(path_bounds(&path), path_bounds(&cutout))
                        {
                            cutouts.push(cutout);
                            changed = true;
                        }
                        objects.push(DocumentObject {
                            id,
                            kind: ObjectKind::Path {
                                path,
                                style,
                                variable_width,
                                cutouts,
                            },
                        });
                        continue;
                    }
                    let effective_radius = radius + style.stroke.width.max(0.0) * 0.5;
                    let Some(parts) = erase_path(&path, centers, effective_radius) else {
                        objects.push(DocumentObject {
                            id,
                            kind: ObjectKind::Path {
                                path,
                                style,
                                variable_width,
                                cutouts,
                            },
                        });
                        continue;
                    };
                    changed = true;
                    for (part_index, part) in parts.into_iter().enumerate() {
                        let part_id = if part_index == 0 {
                            id
                        } else {
                            let new_id = ObjectId(self.next_object_id);
                            self.next_object_id += 1;
                            new_id
                        };
                        objects.push(DocumentObject {
                            id: part_id,
                            kind: ObjectKind::Path {
                                path: part,
                                style,
                                variable_width,
                                cutouts: cutouts.clone(),
                            },
                        });
                    }
                }
                kind => objects.push(DocumentObject { id, kind }),
            }
        }
        self.layers[layer_index].objects = objects;
        if changed {
            self.selected_object = None;
        }
        changed
    }

    fn add_object(&mut self, kind: ObjectKind) -> ObjectId {
        self.record_change();

        let id = ObjectId(self.next_object_id);
        self.next_object_id += 1;
        let object = DocumentObject { id, kind };

        let layer_index = self.selected_layer.unwrap_or(0);
        self.layers[layer_index].visible = true;
        self.layers[layer_index].objects.push(object);
        self.selected_object = Some(id);
        id
    }

    fn apply_paint_dabs(&mut self, dabs: &[PaintDab]) {
        let layer_index = self.selected_layer.unwrap_or(0);
        if !self.layers[layer_index].visible {
            return;
        }
        let clip = match self.properties.canvas_size {
            CanvasSize::FitArtwork => None,
            CanvasSize::Custom { width, height } => Some(DocumentRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            }),
        };
        self.layers[layer_index].paint.apply_dabs(dabs, clip);
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
            properties: self.properties,
        }
    }

    fn restore(&mut self, snapshot: DocumentSnapshot) {
        self.layers = snapshot.layers;
        self.selected_layer = snapshot.selected_layer;
        self.selected_object = snapshot.selected_object;
        self.next_object_id = snapshot.next_object_id;
        self.properties = snapshot.properties;
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

fn path_bounds(path: &BezierPath) -> DocumentRect {
    let Some(first) = path.nodes.first() else {
        return DocumentRect::default();
    };

    let (mut min_x, mut max_x) = (first.position.x, first.position.x);
    let (mut min_y, mut max_y) = (first.position.y, first.position.y);
    for node in &path.nodes {
        for point in [node.position, node.handle_in, node.handle_out] {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
    }

    DocumentRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

fn rects_intersect(first: DocumentRect, second: DocumentRect) -> bool {
    first.x <= second.x + second.width
        && first.x + first.width >= second.x
        && first.y <= second.y + second.height
        && first.y + first.height >= second.y
}

fn translate_point(point: &mut DocumentPoint, delta: DocumentPoint) {
    point.x += delta.x;
    point.y += delta.y;
}

fn scale_point(point: &mut DocumentPoint, old_bounds: DocumentRect, new_bounds: DocumentRect) {
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

fn mirror_point(point: DocumentPoint, center: DocumentPoint) -> DocumentPoint {
    DocumentPoint::new(center.x * 2.0 - point.x, center.y * 2.0 - point.y)
}

fn constrain_opposite_handle(
    node: &mut BezierNode,
    moved_component: NodeComponent,
    independent: bool,
) {
    if independent {
        node.kind = NodeKind::Corner;
        return;
    }

    let (moved, opposite) = match moved_component {
        NodeComponent::HandleIn => (node.handle_in, &mut node.handle_out),
        NodeComponent::HandleOut => (node.handle_out, &mut node.handle_in),
        NodeComponent::Anchor => return,
    };
    match node.kind {
        NodeKind::Corner => {}
        NodeKind::Symmetric => *opposite = mirror_point(moved, node.position),
        NodeKind::Smooth => {
            let x = moved.x - node.position.x;
            let y = moved.y - node.position.y;
            let length = (x * x + y * y).sqrt();
            if length <= f32::EPSILON {
                return;
            }
            let opposite_length = ((opposite.x - node.position.x).powi(2)
                + (opposite.y - node.position.y).powi(2))
            .sqrt();
            *opposite = DocumentPoint::new(
                node.position.x - x / length * opposite_length,
                node.position.y - y / length * opposite_length,
            );
        }
    }
}

fn scale_axis(value: f32, old_start: f32, old_size: f32, new_start: f32, new_size: f32) -> f32 {
    if old_size.abs() <= f32::EPSILON {
        new_start
    } else {
        new_start + (value - old_start) / old_size * new_size
    }
}
