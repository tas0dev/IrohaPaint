use std::cell::Cell;
use std::rc::Rc;

use crate::draw_command::DrawCommand;
use crate::event::{EventContext, EventResult, ViewEvent};
use crate::geometry::{Rect, Size};
use crate::layout::{IntoStackChild, StackChild};
use crate::platform::{ButtonState, KeyCode};
use crate::view::{Constraints, MeasureContext, PaintContext, View};

#[derive(Clone, Default)]
pub struct ModalState {
    open: Rc<Cell<bool>>,
}

impl ModalState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn open(&self) {
        self.open.set(true);
    }

    pub fn close(&self) {
        self.open.set(false);
    }
}

pub struct ModalHost {
    content: StackChild,
    modal: StackChild,
    state: ModalState,
}

impl ModalHost {
    pub fn new<C, M>(content: C, modal: M, state: ModalState) -> Self
    where
        C: IntoStackChild,
        M: IntoStackChild,
    {
        Self {
            content: content.into_stack_child(),
            modal: modal.into_stack_child(),
            state,
        }
    }

    fn modal_bounds(&self, bounds: Rect, context: &mut MeasureContext<'_>) -> Rect {
        let size = self.modal.measure(Constraints::loose(bounds.size), context);
        Rect::new(
            bounds.origin.x + (bounds.size.width - size.width).max(0.0) / 2.0,
            bounds.origin.y + (bounds.size.height - size.height).max(0.0) / 2.0,
            size.width.min(bounds.size.width),
            size.height.min(bounds.size.height),
        )
    }

    fn modal_bounds_for_paint(&self, bounds: Rect, context: &mut PaintContext<'_>) -> Rect {
        let mut measure_context = MeasureContext {
            theme: context.theme,
            typography: context.typography,
            text_measurer: &mut *context.text_measurer,
        };
        self.modal_bounds(bounds, &mut measure_context)
    }

    fn modal_bounds_for_event(&self, bounds: Rect, context: &mut EventContext<'_>) -> Rect {
        let mut measure_context = MeasureContext {
            theme: context.theme,
            typography: context.typography,
            text_measurer: &mut *context.text_measurer,
        };
        self.modal_bounds(bounds, &mut measure_context)
    }
}

impl View for ModalHost {
    fn measure(&self, constraints: Constraints, context: &mut MeasureContext<'_>) -> Size {
        self.content.measure(constraints, context)
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        self.content.paint(bounds, context);
        if !self.state.is_open() {
            return;
        }
        context.display_list.push(DrawCommand::FillRect {
            rect: bounds,
            color: context.theme.colors.background.alpha(0.65),
        });
        let modal_bounds = self.modal_bounds_for_paint(bounds, context);
        self.modal.paint(modal_bounds, context);
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        if !self.state.is_open() {
            let result = self.content.handle_event(bounds, event, context);
            if self.state.is_open() {
                context.request_redraw();
            }
            return result;
        }
        if matches!(
            event,
            ViewEvent::KeyInput {
                key: KeyCode::Escape,
                state: ButtonState::Pressed,
                ..
            }
        ) {
            self.state.close();
            context.request_redraw();
            return EventResult::Consumed;
        }
        let modal_bounds = self.modal_bounds_for_event(bounds, context);
        if event
            .position()
            .is_some_and(|position| modal_bounds.contains(position))
            || event.requires_broadcast()
        {
            let result = self.modal.handle_event(modal_bounds, event, context);
            if !self.state.is_open() {
                context.request_redraw();
            }
            return result;
        }
        EventResult::Consumed
    }
}
