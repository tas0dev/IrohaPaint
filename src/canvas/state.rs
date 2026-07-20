use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use super::coordinates::CanvasTransform;
use super::hit_test::SegmentHit;
use super::interaction::Interaction;
use crate::document::{DocumentRect, NodeComponent, ObjectId};
use viewkit::platform::KeyModifiers;
use viewkit::prelude::{ImageData, Point};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReferenceImage {
    pub image: ImageData,
    pub bounds: DocumentRect,
    pub opacity: f32,
}

#[derive(Clone, Default)]
pub struct CanvasController {
    state: Rc<RefCell<CanvasState>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasState {
    pub transform: CanvasTransform,
    pub transform_initialized: bool,
    pub interaction: Interaction,
    pub active_pen_path: Option<ObjectId>,
    pub selected_nodes: Vec<(ObjectId, usize)>,
    pub hovered_node: Option<(ObjectId, usize, NodeComponent)>,
    pub hovered_segment: Option<(ObjectId, SegmentHit)>,
    pub modifiers: KeyModifiers,
    pub space_pressed: bool,
    pub paint_dirty: Option<DocumentRect>,
    pub pointer_canvas: Option<Point>,
    pub pointer_pressure: Option<f32>,
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

    pub fn reset_for_document(&self) {
        *self.state.borrow_mut() = CanvasState::default();
    }
}
