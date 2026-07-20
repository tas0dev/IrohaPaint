use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use super::coordinates::CanvasTransform;
use super::hit_test::SegmentHit;
use super::interaction::Interaction;
use crate::document::NodeComponent;
use crate::document::ObjectId;
use viewkit::platform::KeyModifiers;

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
}
