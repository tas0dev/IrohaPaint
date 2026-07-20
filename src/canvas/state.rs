use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use super::coordinates::CanvasTransform;
use super::interaction::Interaction;
use crate::document::ObjectId;

#[derive(Clone, Default)]
pub struct CanvasController {
    state: Rc<RefCell<CanvasState>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasState {
    pub transform: CanvasTransform,
    pub interaction: Interaction,
    pub active_pen_path: Option<ObjectId>,
    pub selected_node: Option<(ObjectId, usize)>,
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
