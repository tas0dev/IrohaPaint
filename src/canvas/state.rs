use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use super::coordinates::CanvasTransform;
use super::hit_test::SegmentHit;
use super::interaction::Interaction;
use crate::document::{
    CanvasSize, Document, DocumentPoint, DocumentRect, NodeComponent, ObjectId, ObjectKind,
};
use viewkit::platform::KeyModifiers;
use viewkit::prelude::{ImageData, Point, Rect};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReferenceImage {
    pub image: ImageData,
    pub bounds: DocumentRect,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TouchPoint {
    pub id: u64,
    pub position: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TouchGesture {
    pub start_transform: CanvasTransform,
    pub anchor: DocumentPoint,
    pub start_centroid: Point,
    pub start_distance: f32,
    pub start_angle: f32,
    pub moved: bool,
}

#[derive(Clone, Default)]
pub struct CanvasController {
    state: Rc<RefCell<CanvasState>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasState {
    pub transform: CanvasTransform,
    pub transform_initialized: bool,
    pub viewport_bounds: Rect,
    pub interaction: Interaction,
    pub active_pen_path: Option<ObjectId>,
    pub selected_objects: Vec<ObjectId>,
    pub object_clipboard: Vec<ObjectKind>,
    pub selected_nodes: Vec<(ObjectId, usize)>,
    pub hovered_node: Option<(ObjectId, usize, NodeComponent)>,
    pub hovered_segment: Option<(ObjectId, SegmentHit)>,
    pub modifiers: KeyModifiers,
    pub space_pressed: bool,
    pub rotate_pressed: bool,
    pub view_rotation_drag: Option<(f32, f32)>,
    pub paint_dirty: Option<DocumentRect>,
    pub pointer_canvas: Option<Point>,
    pub pointer_pressure: Option<f32>,
    pub touches: Vec<TouchPoint>,
    pub touch_gesture: Option<TouchGesture>,
    pub reference_image: Option<ReferenceImage>,
}

impl CanvasController {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get(&self) -> Ref<'_, CanvasState> {
        self.state.borrow()
    }

    pub(crate) fn get_mut(&self) -> RefMut<'_, CanvasState> {
        self.state.borrow_mut()
    }

    pub fn set_reference_image(&self, image: ImageData, canvas_width: f32, canvas_height: f32) {
        let scale = (canvas_width / image.width() as f32)
            .min(canvas_height / image.height() as f32)
            .min(1.0)
            * 0.8;
        let width = image.width() as f32 * scale;
        let height = image.height() as f32 * scale;
        self.state.borrow_mut().reference_image = Some(ReferenceImage {
            image,
            bounds: DocumentRect {
                x: (canvas_width - width) * 0.5,
                y: (canvas_height - height) * 0.5,
                width,
                height,
            },
            opacity: 0.5,
        });
    }

    pub fn remove_reference_image(&self) {
        self.state.borrow_mut().reference_image = None;
    }

    pub fn has_reference_image(&self) -> bool {
        self.state.borrow().reference_image.is_some()
    }

    pub fn is_view_flipped(&self) -> bool {
        self.state.borrow().transform.is_flipped_horizontal()
    }

    pub fn set_zoom(&self, zoom: f32) {
        let mut state = self.state.borrow_mut();
        let bounds = state.viewport_bounds;
        if bounds.is_empty() {
            return;
        }
        let center = Point::new(
            bounds.origin.x + bounds.size.width * 0.5,
            bounds.origin.y + bounds.size.height * 0.5,
        );
        state.transform.set_zoom_at(zoom, center, bounds);
    }

    pub fn fit_canvas(&self, document: &Document) -> bool {
        let CanvasSize::Custom { width, height } = document.properties().canvas_size else {
            return false;
        };
        let mut state = self.state.borrow_mut();
        let bounds = state.viewport_bounds;
        let fitted = state.transform.fit_canvas(width, height, bounds);
        state.transform_initialized |= fitted;
        fitted
    }

    pub fn toggle_view_flip(&self) {
        self.state.borrow_mut().transform.toggle_horizontal_flip();
    }

    pub fn rotate_view(&self, degrees: f32) {
        let mut state = self.state.borrow_mut();
        let rotation = state.transform.rotation() + degrees.to_radians();
        state.transform.set_rotation(rotation);
    }

    pub fn reset_view_rotation(&self) {
        self.state.borrow_mut().transform.set_rotation(0.0);
    }

    pub(crate) fn center_on_document(&self, point: DocumentPoint) {
        let mut state = self.state.borrow_mut();
        let bounds = state.viewport_bounds;
        state.transform.center_on(point, bounds);
    }

    pub fn reset_for_document(&self) {
        let clipboard = self.state.borrow().object_clipboard.clone();
        *self.state.borrow_mut() = CanvasState {
            object_clipboard: clipboard,
            ..CanvasState::default()
        };
    }

    pub fn selection_count(&self) -> usize {
        self.state.borrow().selected_objects.len()
    }

    pub fn clear_selection(&self) {
        let mut state = self.state.borrow_mut();
        state.selected_objects.clear();
        state.selected_nodes.clear();
        state.hovered_node = None;
        state.hovered_segment = None;
        state.active_pen_path = None;
    }

    pub fn copy_selection(&self, document: &Document) -> bool {
        let selected = self.state.borrow().selected_objects.clone();
        let kinds = document
            .selected_layer()
            .and_then(|index| document.layers().get(index))
            .into_iter()
            .flat_map(|layer| layer.objects())
            .filter(|object| selected.contains(&object.id()))
            .map(|object| object.kind().clone())
            .collect::<Vec<_>>();
        if kinds.is_empty() {
            return false;
        }
        self.state.borrow_mut().object_clipboard = kinds;
        true
    }

    pub fn duplicate_selection(&self, document: &mut Document) -> bool {
        let selected = self.state.borrow().selected_objects.clone();
        let inserted = document.duplicate_objects(&selected, DocumentPoint::new(12.0, 12.0));
        self.set_object_selection(inserted)
    }

    pub fn paste(&self, document: &mut Document) -> bool {
        let clipboard = self.state.borrow().object_clipboard.clone();
        let inserted = document.insert_object_kinds(&clipboard, DocumentPoint::new(12.0, 12.0));
        self.set_object_selection(inserted)
    }

    pub fn flip_selection(&self, document: &mut Document, horizontal: bool) -> bool {
        let selected = self.state.borrow().selected_objects.clone();
        let Some(bounds) = selected
            .iter()
            .filter_map(|id| document.object(*id).map(|object| object.bounds()))
            .reduce(union_rects)
        else {
            return false;
        };
        let center = DocumentPoint::new(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        let replacements = selected
            .iter()
            .filter_map(|id| {
                document.object(*id).map(|object| {
                    (
                        *id,
                        super::interaction::flipped_kind(object.kind(), center, horizontal),
                    )
                })
            })
            .collect::<Vec<_>>();
        document.replace_object_kinds(&replacements)
    }

    pub fn move_selection_forward(&self, document: &mut Document) -> bool {
        document.move_objects_forward(&self.state.borrow().selected_objects)
    }

    pub fn move_selection_backward(&self, document: &mut Document) -> bool {
        document.move_objects_backward(&self.state.borrow().selected_objects)
    }

    pub fn delete_selection(&self, document: &mut Document) -> bool {
        let selected = self.state.borrow().selected_objects.clone();
        if !document.delete_objects(&selected) {
            return false;
        }
        let mut state = self.state.borrow_mut();
        state.selected_objects.clear();
        state.selected_nodes.clear();
        true
    }

    pub fn select_all_objects(&self, document: &mut Document) -> bool {
        let Some(layer) = document
            .selected_layer()
            .and_then(|index| document.layers().get(index))
        else {
            return false;
        };
        let selected = layer.objects().iter().map(|object| object.id()).collect();
        self.set_object_selection_with_document(selected, document)
    }

    fn set_object_selection(&self, selected: Vec<ObjectId>) -> bool {
        if selected.is_empty() {
            return false;
        }
        let mut state = self.state.borrow_mut();
        state.selected_objects = selected;
        state.selected_nodes.clear();
        true
    }

    fn set_object_selection_with_document(
        &self,
        selected: Vec<ObjectId>,
        document: &mut Document,
    ) -> bool {
        if !self.set_object_selection(selected.clone()) {
            return false;
        }
        document.select_object(selected.last().copied());
        true
    }
}

fn union_rects(first: DocumentRect, second: DocumentRect) -> DocumentRect {
    let left = first.x.min(second.x);
    let top = first.y.min(second.y);
    let right = (first.x + first.width).max(second.x + second.width);
    let bottom = (first.y + first.height).max(second.y + second.height);
    DocumentRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}
