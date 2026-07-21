//! リストコンポーネント

use crate::event::{EventContext, EventResult, ViewEvent};
use crate::geometry::{Point, Rect, Size};
use crate::layout::{StackAlignment, StackGap, ViewExt};
use crate::theme::{Color, Theme};
use crate::view::{Constraints, MeasureContext, PaintContext, View};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{
    Button, ButtonInteractionState, ButtonStyle, HStack, Icon, IconName, Padding, Text, VStack,
    ZStackAlignment,
};

pub struct ListRow {
    title: String,
    subtitle: Option<String>,
    trailing: Option<String>,

    selected: bool,
    enabled: bool,

    interaction: ButtonInteractionState,
    on_select: Option<Rc<RefCell<Box<dyn FnMut()>>>>,
    on_drag: Option<Rc<RefCell<Box<dyn FnMut(f32)>>>>,
    on_drop: Option<Rc<RefCell<Box<dyn FnMut(Option<u64>, f32)>>>>,
    drag_drop: Option<ListDragDropState>,
    drop_target: Option<u64>,
    drag_start_y: Rc<Cell<Option<f32>>>,
    dragging: Rc<Cell<bool>>,
    icon: Option<IconName>,
}

#[derive(Clone, Default)]
pub struct ListDragDropState {
    inner: Rc<RefCell<ListDragDropInner>>,
}

#[derive(Default)]
struct ListDragDropInner {
    targets: Vec<(u64, Rect)>,
    hovered: Option<u64>,
}

impl ListDragDropState {
    pub fn new() -> Self {
        Self::default()
    }

    fn register_target(&self, id: u64, bounds: Rect) {
        let mut inner = self.inner.borrow_mut();
        if let Some(target) = inner.targets.iter_mut().find(|target| target.0 == id) {
            target.1 = bounds;
        } else {
            inner.targets.push((id, bounds));
        }
    }

    fn target_at(&self, position: Point) -> Option<u64> {
        self.inner
            .borrow()
            .targets
            .iter()
            .rev()
            .find_map(|(id, bounds)| bounds.contains(position).then_some(*id))
    }

    fn set_hovered_at(&self, position: Point) {
        let hovered = self.target_at(position);
        self.inner.borrow_mut().hovered = hovered;
    }

    fn is_hovered(&self, id: u64) -> bool {
        self.inner.borrow().hovered == Some(id)
    }

    fn clear_hovered(&self) {
        self.inner.borrow_mut().hovered = None;
    }
}

impl ListRow {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            trailing: None,

            selected: false,
            enabled: true,

            interaction: ButtonInteractionState::new(),
            on_select: None,
            on_drag: None,
            on_drop: None,
            drag_drop: None,
            drop_target: None,
            drag_start_y: Rc::new(Cell::new(None)),
            dragging: Rc::new(Cell::new(false)),
            icon: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_select(mut self, callback: impl FnMut() + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    /// Invokes `callback` with the vertical pointer delta when the row is dragged.
    pub fn on_drag(mut self, callback: impl FnMut(f32) + 'static) -> Self {
        self.on_drag = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    pub fn drop_target(mut self, state: ListDragDropState, id: u64) -> Self {
        self.drag_drop = Some(state);
        self.drop_target = Some(id);
        self
    }

    pub fn on_drop(
        mut self,
        state: ListDragDropState,
        callback: impl FnMut(Option<u64>, f32) + 'static,
    ) -> Self {
        self.drag_drop = Some(state);
        self.on_drop = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    pub fn interaction(&self) -> &ButtonInteractionState {
        &self.interaction
    }

    fn content_view(&self, theme: &Theme) -> Padding<HStack> {
        let mut labels = VStack::new()
            .alignment(StackAlignment::Stretch)
            .gap(StackGap::None)
            .child(
                Text::new(self.title.clone())
                    .font_size(13.0)
                    .line_height(20.0)
                    .weight(600)
                    .color(theme.colors.text_primary),
            );

        if let Some(subtitle) = self.subtitle.as_ref() {
            labels = labels.child(
                Text::new(subtitle.clone())
                    .font_size(11.0)
                    .line_height(18.0)
                    .color(theme.colors.text_secondary),
            );
        }

        let mut row = HStack::new()
            .alignment(StackAlignment::Center)
            .gap(StackGap::Medium);

        if let Some(icon) = self.icon {
            row = row.child(
                Icon::new(icon)
                    .size(20.0)
                    .color(Color::BLACK)
                    .frame(24.0, 24.0),
            );
        }

        row = row.child(labels.layout().flex_grow(1.0));

        if let Some(trailing) = self.trailing.as_ref() {
            row = row.child(
                Text::new(trailing.clone())
                    .font_size(11.0)
                    .line_height(18.0)
                    .color(theme.colors.text_secondary),
            );
        }

        Padding::symmetric(12.0, 8.0).content(row)
    }

    fn button(&self, theme: &Theme) -> Button {
        let mut button = Button::with_interaction(self.interaction.clone())
            .style(
                if self.selected
                    || self.drop_target.is_some_and(|id| {
                        self.drag_drop
                            .as_ref()
                            .is_some_and(|state| state.is_hovered(id))
                    })
                {
                    ButtonStyle::Standard
                } else {
                    ButtonStyle::Ghost
                },
            )
            .alignment(ZStackAlignment::Leading)
            .enabled(self.enabled)
            .content(self.content_view(theme));

        if let Some(on_select) = self.on_select.as_ref() {
            let on_select = Rc::clone(on_select);
            button = button.on_click(move || {
                on_select.borrow_mut()();
            });
        }

        button
    }
}

impl View for ListRow {
    fn measure(&self, constraints: Constraints, context: &mut MeasureContext<'_>) -> Size {
        self.button(context.theme).measure(constraints, context)
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        if let (Some(state), Some(id)) = (self.drag_drop.as_ref(), self.drop_target) {
            state.register_target(id, bounds);
        }
        self.button(context.theme).paint(bounds, context);
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        if self.on_drag.is_some() || self.on_drop.is_some() {
            match event {
                ViewEvent::PointerPressed { position, button }
                    if *button == crate::platform::PointerButton::Primary
                        && bounds.contains(*position) =>
                {
                    self.drag_start_y.set(Some(position.y));
                    self.dragging.set(false);
                    if let Some(state) = self.drag_drop.as_ref() {
                        state.clear_hovered();
                    }
                }
                ViewEvent::PointerMoved { position } => {
                    if let Some(start_y) = self.drag_start_y.get()
                        && (position.y - start_y).abs() >= 6.0
                    {
                        self.dragging.set(true);
                        self.interaction.reset();
                        if let Some(state) = self.drag_drop.as_ref() {
                            state.set_hovered_at(*position);
                        }
                        context.request_redraw();
                        return EventResult::Consumed;
                    }
                }
                ViewEvent::PointerReleased { position, button }
                    if *button == crate::platform::PointerButton::Primary =>
                {
                    let start_y = self.drag_start_y.take();
                    if self.dragging.replace(false) {
                        self.interaction.reset();
                        if let Some(start_y) = start_y {
                            let target = self
                                .drag_drop
                                .as_ref()
                                .and_then(|state| state.target_at(*position));
                            if let Some(callback) = self.on_drop.as_ref() {
                                callback.borrow_mut()(target, position.y - start_y);
                            } else if let Some(callback) = self.on_drag.as_ref() {
                                callback.borrow_mut()(position.y - start_y);
                            }
                        }
                        if let Some(state) = self.drag_drop.as_ref() {
                            state.clear_hovered();
                        }
                        context.request_redraw();
                        return EventResult::Consumed;
                    }
                }
                ViewEvent::FocusChanged { focused: false } => {
                    self.drag_start_y.set(None);
                    self.dragging.set(false);
                    self.interaction.reset();
                    if let Some(state) = self.drag_drop.as_ref() {
                        state.clear_hovered();
                    }
                }
                _ => {}
            }
        }
        let result = self
            .button(context.theme)
            .handle_event(bounds, event, context);

        if self.interaction.take_clicked() {
            if let Some(callback) = self.on_select.as_ref() {
                callback.borrow_mut();
            }

            return EventResult::Consumed;
        }

        result
    }
}
