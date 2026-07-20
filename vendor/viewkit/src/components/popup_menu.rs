use std::cell::Cell;
use std::rc::Rc;

use crate::event::{EventContext, EventResult, ViewEvent};
use crate::geometry::{Point, Rect, Size};
use crate::layout::{IntoStackChild, StackChild};
use crate::platform::PointerButton;
use crate::view::{Constraints, MeasureContext, PaintContext, View};

use super::{Button, ButtonStyle};

#[derive(Clone, Default)]
pub struct PopupMenuState {
    anchor: Rc<Cell<Option<Point>>>,
}

impl PopupMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.anchor.get().is_some()
    }

    pub fn close(&self) {
        self.anchor.set(None);
    }

    fn open_at(&self, anchor: Point) {
        self.anchor.set(Some(anchor));
    }
}

pub struct PopupMenuButton {
    button: Button,
    state: PopupMenuState,
}

pub struct PopupMenuTrigger {
    content: StackChild,
    state: PopupMenuState,
    should_open: Box<dyn Fn() -> bool>,
    open_on_release: Cell<bool>,
}

impl PopupMenuTrigger {
    pub fn new<C>(content: C, state: PopupMenuState) -> Self
    where
        C: IntoStackChild,
    {
        Self {
            content: content.into_stack_child(),
            state,
            should_open: Box::new(|| true),
            open_on_release: Cell::new(false),
        }
    }

    pub fn when(mut self, predicate: impl Fn() -> bool + 'static) -> Self {
        self.should_open = Box::new(predicate);
        self
    }
}

impl View for PopupMenuTrigger {
    fn measure(&self, constraints: Constraints, context: &mut MeasureContext<'_>) -> Size {
        self.content.measure(constraints, context)
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        self.content.paint(bounds, context);
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        if let ViewEvent::PointerPressed {
            position,
            button: PointerButton::Primary,
        } = event
            && bounds.contains(*position)
        {
            self.open_on_release.set((self.should_open)());
        }
        let result = self.content.handle_event(bounds, event, context);
        match event {
            ViewEvent::PointerReleased {
                position,
                button: PointerButton::Primary,
            } => {
                if result.is_consumed()
                    && bounds.contains(*position)
                    && self.open_on_release.replace(false)
                {
                    self.state.open_at(Point::new(
                        bounds.origin.x,
                        bounds.origin.y + bounds.size.height,
                    ));
                    self.content
                        .handle_event(bounds, &ViewEvent::PointerLeft, context);
                    context.request_redraw();
                }
            }
            ViewEvent::PointerLeft => self.open_on_release.set(false),
            _ => {}
        }
        result
    }
}

impl PopupMenuButton {
    pub fn new(label: impl Into<String>, state: PopupMenuState) -> Self {
        Self {
            button: Button::new(label),
            state,
        }
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.button = self.button.style(style);
        self
    }
}

impl View for PopupMenuButton {
    fn measure(&self, constraints: Constraints, context: &mut MeasureContext<'_>) -> Size {
        self.button.measure(constraints, context)
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        self.button.paint(bounds, context);
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        let result = self.button.handle_event(bounds, event, context);
        if result.is_consumed()
            && let ViewEvent::PointerReleased {
                position,
                button: PointerButton::Primary,
            } = event
            && bounds.contains(*position)
        {
            self.state.open_at(Point::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height,
            ));
            self.button
                .handle_event(bounds, &ViewEvent::PointerLeft, context);
            context.request_redraw();
        }
        result
    }
}

pub struct PopupMenuHost {
    content: StackChild,
    menu: StackChild,
    state: PopupMenuState,
    dismissing: Cell<Option<PointerButton>>,
}

impl PopupMenuHost {
    pub fn new<C, M>(content: C, menu: M, state: PopupMenuState) -> Self
    where
        C: IntoStackChild,
        M: IntoStackChild,
    {
        Self {
            content: content.into_stack_child(),
            menu: menu.into_stack_child(),
            state,
            dismissing: Cell::new(None),
        }
    }

    fn menu_bounds(
        &self,
        bounds: Rect,
        anchor: Point,
        context: &mut MeasureContext<'_>,
    ) -> Rect {
        let size = self.menu.measure(Constraints::loose(bounds.size), context);
        let maximum_x = (bounds.origin.x + bounds.size.width - size.width).max(bounds.origin.x);
        let maximum_y = (bounds.origin.y + bounds.size.height - size.height).max(bounds.origin.y);
        Rect::new(
            anchor.x.clamp(bounds.origin.x, maximum_x),
            anchor.y.clamp(bounds.origin.y, maximum_y),
            size.width,
            size.height,
        )
    }

    fn menu_bounds_for_paint(
        &self,
        bounds: Rect,
        anchor: Point,
        context: &mut PaintContext<'_>,
    ) -> Rect {
        let mut measure_context = MeasureContext {
            theme: context.theme,
            typography: context.typography,
            text_measurer: &mut *context.text_measurer,
        };
        self.menu_bounds(bounds, anchor, &mut measure_context)
    }

    fn menu_bounds_for_event(
        &self,
        bounds: Rect,
        anchor: Point,
        context: &mut EventContext<'_>,
    ) -> Rect {
        let mut measure_context = MeasureContext {
            theme: context.theme,
            typography: context.typography,
            text_measurer: &mut *context.text_measurer,
        };
        self.menu_bounds(bounds, anchor, &mut measure_context)
    }
}

impl View for PopupMenuHost {
    fn measure(&self, constraints: Constraints, context: &mut MeasureContext<'_>) -> Size {
        self.content.measure(constraints, context)
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        self.content.paint(bounds, context);
        if let Some(anchor) = self.state.anchor.get() {
            let menu_bounds = self.menu_bounds_for_paint(bounds, anchor, context);
            self.menu.paint(menu_bounds, context);
        }
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        if let Some(dismiss_button) = self.dismissing.get() {
            if let ViewEvent::PointerReleased { button, .. } = event
                && *button == dismiss_button
            {
                self.dismissing.set(None);
            }
            return EventResult::Consumed;
        }

        let Some(anchor) = self.state.anchor.get() else {
            return self.content.handle_event(bounds, event, context);
        };
        let menu_bounds = self.menu_bounds_for_event(bounds, anchor, context);
        match event {
            ViewEvent::PointerPressed { position, button }
                if !menu_bounds.contains(*position) =>
            {
                self.menu
                    .handle_event(menu_bounds, &ViewEvent::PointerLeft, context);
                self.state.close();
                self.dismissing.set(Some(*button));
                context.request_redraw();
                EventResult::Consumed
            }
            ViewEvent::PointerReleased {
                position,
                button: PointerButton::Primary,
            } => {
                let result = self.menu.handle_event(menu_bounds, event, context);
                if result.is_consumed() {
                    self.state.close();
                    context.request_redraw();
                }
                if menu_bounds.contains(*position) {
                    EventResult::Consumed
                } else {
                    result
                }
            }
            ViewEvent::PointerPressed { position, .. }
            | ViewEvent::PointerMoved { position }
                if menu_bounds.contains(*position) =>
            {
                self.menu.handle_event(menu_bounds, event, context)
            }
            ViewEvent::PointerMoved { .. } => {
                self.menu
                    .handle_event(menu_bounds, &ViewEvent::PointerLeft, context);
                EventResult::Consumed
            }
            ViewEvent::PointerLeft => self.menu.handle_event(menu_bounds, event, context),
            ViewEvent::FocusChanged { focused: false } => {
                self.state.close();
                context.request_redraw();
                EventResult::Consumed
            }
            ViewEvent::PointerFocusRequested { .. } => EventResult::Consumed,
            _ if event.requires_broadcast() => self.menu.handle_event(menu_bounds, event, context),
            _ => EventResult::Consumed,
        }
    }
}
