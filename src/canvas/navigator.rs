use std::cell::Cell;
use std::rc::Rc;

use viewkit::draw_command::DrawCommand;
use viewkit::event::{EventContext, EventResult, ViewEvent};
use viewkit::platform::PointerButton;
use viewkit::prelude::*;
use viewkit::view::{Constraints, MeasureContext, PaintContext};

use super::coordinates::{CanvasTransform, MAX_ZOOM, MIN_ZOOM};
use super::interaction::Interaction;
use super::paint::{NodePresentation, paint_editor_canvas};
use super::state::CanvasController;
use crate::document::{CanvasSize, Document, DocumentRect};
use crate::editor::EditorTool;

const ZOOM_CONTROL_HEIGHT: f32 = 34.0;
const THUMB_SIZE: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavigatorInteraction {
    Idle,
    Panning,
    Zooming,
}

pub struct NavigatorCanvas {
    document: State<Document>,
    controller: CanvasController,
    interaction: Rc<Cell<NavigatorInteraction>>,
}

impl NavigatorCanvas {
    pub fn new(document: State<Document>, controller: CanvasController) -> Self {
        Self {
            document,
            controller,
            interaction: Rc::new(Cell::new(NavigatorInteraction::Idle)),
        }
    }

    fn thumbnail_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            (bounds.size.height - ZOOM_CONTROL_HEIGHT).max(0.0),
        )
    }

    fn slider_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + bounds.size.height - 18.0,
            (bounds.size.width - 20.0).max(0.0),
            4.0,
        )
    }

    fn navigator_transform(document: &Document, bounds: Rect) -> Option<CanvasTransform> {
        let CanvasSize::Custom { width, height } = document.properties().canvas_size else {
            return None;
        };
        let mut transform = CanvasTransform::default();
        transform
            .fit_canvas(width, height, bounds)
            .then_some(transform)
    }

    fn zoom_progress(zoom: f32) -> f32 {
        (zoom.clamp(MIN_ZOOM, MAX_ZOOM) / MIN_ZOOM).ln() / (MAX_ZOOM / MIN_ZOOM).ln()
    }

    fn zoom_at_position(&self, position: Point, bounds: Rect) {
        let slider = Self::slider_bounds(bounds);
        let progress =
            ((position.x - slider.origin.x) / slider.size.width.max(1.0)).clamp(0.0, 1.0);
        self.controller
            .set_zoom(MIN_ZOOM * (MAX_ZOOM / MIN_ZOOM).powf(progress));
    }

    fn pan_to_position(&self, position: Point, bounds: Rect) {
        let thumbnail = Self::thumbnail_bounds(bounds);
        let document = self.document.get();
        let Some(transform) = Self::navigator_transform(&document, thumbnail) else {
            return;
        };
        self.controller
            .center_on_document(transform.canvas_to_document(position, thumbnail));
    }
}

impl View for NavigatorCanvas {
    fn measure(&self, constraints: Constraints, _context: &mut MeasureContext<'_>) -> Size {
        constraints.constrain(Size::new(220.0, 190.0))
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        if bounds.is_empty() {
            return;
        }
        let thumbnail = Self::thumbnail_bounds(bounds);
        let document = self.document.get();
        let Some(navigator_transform) = Self::navigator_transform(&document, thumbnail) else {
            Text::new("Set a canvas size to use Navigator").paint(thumbnail, context);
            return;
        };
        let state = self.controller.get();
        let idle = Interaction::Idle;
        paint_editor_canvas(
            &document,
            navigator_transform,
            &idle,
            state.reference_image.as_ref(),
            EditorTool::Select,
            NodePresentation {
                selected_objects: &[],
                selected: &[],
                hovered: None,
                segment: None,
                brush_cursor: None,
            },
            thumbnail,
            context,
        );

        if !state.viewport_bounds.is_empty() {
            let first = state
                .transform
                .canvas_to_document(state.viewport_bounds.origin, state.viewport_bounds);
            let second = state.transform.canvas_to_document(
                Point::new(
                    state.viewport_bounds.origin.x + state.viewport_bounds.size.width,
                    state.viewport_bounds.origin.y + state.viewport_bounds.size.height,
                ),
                state.viewport_bounds,
            );
            let visible = navigator_transform
                .document_rect_to_canvas(DocumentRect::from_points(first, second), thumbnail);
            context.display_list.push(DrawCommand::FillRect {
                rect: visible,
                color: context.theme.colors.accent_soft,
            });
            context.display_list.push(DrawCommand::StrokeRect {
                rect: visible,
                color: context.theme.colors.accent,
                width: 1.5,
            });
        }

        let slider = Self::slider_bounds(bounds);
        context.display_list.push(DrawCommand::FillRect {
            rect: slider,
            color: context.theme.colors.border_strong,
        });
        let thumb_x =
            slider.origin.x + slider.size.width * Self::zoom_progress(state.transform.zoom());
        context.display_list.push(DrawCommand::FillEllipse {
            rect: Rect::new(
                thumb_x - THUMB_SIZE * 0.5,
                slider.origin.y + slider.size.height * 0.5 - THUMB_SIZE * 0.5,
                THUMB_SIZE,
                THUMB_SIZE,
            ),
            color: context.theme.colors.accent,
        });
        Text::new(format!("{:.0}%", state.transform.zoom() * 100.0))
            .font_size(11.0)
            .color(context.theme.colors.text_secondary)
            .paint(
                Rect::new(
                    bounds.origin.x,
                    thumbnail.origin.y + thumbnail.size.height,
                    bounds.size.width,
                    16.0,
                ),
                context,
            );
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        match event {
            ViewEvent::PointerPressed { position, button }
                if *button == PointerButton::Primary && bounds.contains(*position) =>
            {
                if Self::slider_bounds(bounds)
                    .expanded(10.0)
                    .contains(*position)
                {
                    self.interaction.set(NavigatorInteraction::Zooming);
                    self.zoom_at_position(*position, bounds);
                } else if Self::thumbnail_bounds(bounds).contains(*position) {
                    self.interaction.set(NavigatorInteraction::Panning);
                    self.pan_to_position(*position, bounds);
                }
                context.request_redraw();
                EventResult::Consumed
            }
            ViewEvent::PointerMoved { position } => match self.interaction.get() {
                NavigatorInteraction::Panning => {
                    self.pan_to_position(*position, bounds);
                    context.request_redraw();
                    EventResult::Consumed
                }
                NavigatorInteraction::Zooming => {
                    self.zoom_at_position(*position, bounds);
                    context.request_redraw();
                    EventResult::Consumed
                }
                NavigatorInteraction::Idle => EventResult::Ignored,
            },
            ViewEvent::PointerReleased { button, .. }
                if *button == PointerButton::Primary
                    && self.interaction.get() != NavigatorInteraction::Idle =>
            {
                self.interaction.set(NavigatorInteraction::Idle);
                EventResult::Consumed
            }
            ViewEvent::FocusChanged { focused: false } | ViewEvent::PointerLeft => {
                self.interaction.set(NavigatorInteraction::Idle);
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
