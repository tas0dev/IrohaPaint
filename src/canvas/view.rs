use viewkit::event::{EventContext, EventResult, ViewEvent};
use viewkit::platform::PointerButton;
use viewkit::prelude::{CursorIcon, Point, Rect, Size, State, View};
use viewkit::view::{Constraints, MeasureContext, PaintContext};

use crate::document::{BezierNode, Document, DocumentPoint, DocumentRect, ObjectKind};
use crate::editor::EditorTool;

use super::hit_test::{is_first_path_node, object_at, path_node_at, resize_handle_at};
use super::interaction::{Interaction, ShapeDraftKind};
use super::paint::paint_editor_canvas;
use super::state::CanvasController;

const HIT_TOLERANCE: f32 = 6.0;
const MIN_DRAG_SIZE: f32 = 2.0;

pub struct EditorCanvas {
    document: State<Document>,
    active_tool: State<EditorTool>,
    controller: CanvasController,
}

impl EditorCanvas {
    pub fn new(
        document: State<Document>,
        active_tool: State<EditorTool>,
        controller: CanvasController,
    ) -> Self {
        Self {
            document,
            active_tool,
            controller,
        }
    }

    fn handle_primary_press(&self, position: Point, bounds: Rect) {
        let transform = self.controller.get().transform;
        let document_point = transform.canvas_to_document(position, bounds);
        let tolerance = HIT_TOLERANCE / transform.zoom();

        match self.active_tool.get() {
            EditorTool::Select => {
                let current_document = self.document.get();
                if let Some(id) = current_document.selected_object()
                    && let Some(object) = current_document.object(id)
                    && let Some(handle) =
                        resize_handle_at(object.bounds(), document_point, tolerance)
                {
                    self.controller.get_mut().interaction = Interaction::Resizing {
                        id,
                        original: object.kind().clone(),
                        anchor: handle.opposite(object.bounds()),
                        current: document_point,
                        handle,
                    };
                    return;
                }

                let hit = object_at(&current_document, document_point, tolerance);
                let original = hit.and_then(|id| {
                    current_document
                        .object(id)
                        .map(|object| (id, object.kind().clone()))
                });
                drop(current_document);

                self.document.update(|document| document.select_object(hit));
                self.controller.get_mut().interaction = match original {
                    Some((id, original)) => Interaction::Moving {
                        id,
                        original,
                        start: document_point,
                        current: document_point,
                    },
                    None => Interaction::Idle,
                };
            }
            EditorTool::NodeEdit => {
                let current_document = self.document.get();
                let selected_id = current_document.selected_object();
                let selected_node =
                    self.controller
                        .get()
                        .selected_node
                        .and_then(|(object_id, index)| {
                            (Some(object_id) == selected_id).then_some(index)
                        });

                if let Some(id) = selected_id
                    && let Some(object) = current_document.object(id)
                    && let ObjectKind::Path { path } = object.kind()
                    && let Some(hit) = path_node_at(path, selected_node, document_point, tolerance)
                {
                    let original = object.kind().clone();
                    drop(current_document);
                    let mut state = self.controller.get_mut();
                    state.selected_node = Some((id, hit.index));
                    state.interaction = Interaction::EditingPathNode {
                        id,
                        original,
                        node_index: hit.index,
                        component: hit.component,
                        current: document_point,
                    };
                    return;
                }

                let hit = object_at(&current_document, document_point, tolerance);
                drop(current_document);
                self.document.update(|document| document.select_object(hit));
                let mut state = self.controller.get_mut();
                state.selected_node = None;
                state.interaction = Interaction::Idle;
            }
            EditorTool::Rectangle => {
                self.controller.get_mut().interaction = Interaction::DrawingShape {
                    kind: ShapeDraftKind::Rectangle,
                    start: document_point,
                    current: document_point,
                };
            }
            EditorTool::Ellipse => {
                self.controller.get_mut().interaction = Interaction::DrawingShape {
                    kind: ShapeDraftKind::Ellipse,
                    start: document_point,
                    current: document_point,
                };
            }
            EditorTool::Pen => {
                let active_path = self.controller.get().active_pen_path;
                let current_document = self.document.get();
                let active_object = active_path.and_then(|id| {
                    current_document
                        .object(id)
                        .map(|object| (id, object.kind().clone()))
                });

                if let Some((id, ObjectKind::Path { path })) = &active_object
                    && !path.is_closed()
                    && path.nodes().len() > 1
                    && is_first_path_node(path, document_point, tolerance)
                {
                    self.controller.get_mut().interaction = Interaction::ClosingPath { id: *id };
                    return;
                }

                let (path_id, original) = match active_object {
                    Some((id, original @ ObjectKind::Path { .. })) => (Some(id), Some(original)),
                    _ => (None, None),
                };
                let mut state = self.controller.get_mut();
                if path_id.is_none() {
                    state.active_pen_path = None;
                }
                state.interaction = Interaction::PlacingPathNode {
                    path_id,
                    original,
                    position: document_point,
                    handle_out: document_point,
                };
            }
        }
    }

    fn handle_pointer_move(&self, position: Point, bounds: Rect) -> bool {
        let mut state = self.controller.get_mut();
        let transform = state.transform;
        let document_point = transform.canvas_to_document(position, bounds);

        if let Interaction::Panning {
            start_canvas,
            start_pan,
        } = state.interaction
        {
            state.transform.set_pan(Point::new(
                start_pan.x + position.x - start_canvas.x,
                start_pan.y + position.y - start_canvas.y,
            ));
            return true;
        }

        match &mut state.interaction {
            Interaction::DrawingShape { current, .. }
            | Interaction::Moving { current, .. }
            | Interaction::Resizing { current, .. }
            | Interaction::EditingPathNode { current, .. } => {
                *current = document_point;
                true
            }
            Interaction::PlacingPathNode { handle_out, .. } => {
                *handle_out = document_point;
                true
            }
            Interaction::ClosingPath { .. } => true,
            Interaction::Panning { .. } => unreachable!(),
            Interaction::Idle => false,
        }
    }

    fn finish_interaction(&self) {
        let interaction = {
            let mut state = self.controller.get_mut();
            std::mem::take(&mut state.interaction)
        };
        let zoom = self.controller.get().transform.zoom();

        match interaction {
            Interaction::DrawingShape {
                kind,
                start,
                current,
            } => {
                let bounds = DocumentRect::from_points(start, current);
                if bounds.width * zoom < MIN_DRAG_SIZE || bounds.height * zoom < MIN_DRAG_SIZE {
                    return;
                }
                self.document.update(|document| match kind {
                    ShapeDraftKind::Rectangle => {
                        document.add_rectangle(bounds);
                    }
                    ShapeDraftKind::Ellipse => {
                        document.add_ellipse(bounds);
                    }
                });
            }
            Interaction::PlacingPathNode {
                path_id,
                position,
                handle_out,
                ..
            } => {
                let handle_delta_x = (handle_out.x - position.x) * zoom;
                let handle_delta_y = (handle_out.y - position.y) * zoom;
                let node = if handle_delta_x * handle_delta_x + handle_delta_y * handle_delta_y
                    < MIN_DRAG_SIZE * MIN_DRAG_SIZE
                {
                    BezierNode::corner(position)
                } else {
                    BezierNode::smooth(position, handle_out)
                };
                let id = if let Some(id) = path_id {
                    self.document
                        .update(|document| document.append_path_node(id, node));
                    id
                } else {
                    self.document.update(|document| document.add_path(node))
                };
                let node_index = self
                    .document
                    .get()
                    .object(id)
                    .and_then(|object| {
                        let ObjectKind::Path { path } = object.kind() else {
                            return None;
                        };
                        path.nodes().len().checked_sub(1)
                    })
                    .unwrap_or(0);
                let mut state = self.controller.get_mut();
                state.active_pen_path = Some(id);
                state.selected_node = Some((id, node_index));
            }
            Interaction::ClosingPath { id } => {
                self.document.update(|document| document.close_path(id));
                self.controller.get_mut().active_pen_path = None;
            }
            Interaction::EditingPathNode {
                id,
                node_index,
                component,
                current,
                ..
            } => {
                self.document
                    .update(|document| document.edit_path_node(id, node_index, component, current));
            }
            Interaction::Moving {
                id, start, current, ..
            } => {
                let delta = DocumentPoint::new(current.x - start.x, current.y - start.y);
                if delta.x.abs() * zoom >= 0.5 || delta.y.abs() * zoom >= 0.5 {
                    self.document
                        .update(|document| document.translate_object(id, delta));
                }
            }
            Interaction::Resizing {
                id,
                anchor,
                current,
                ..
            } => {
                let bounds = DocumentRect::from_points(anchor, current);
                if bounds.width * zoom >= MIN_DRAG_SIZE && bounds.height * zoom >= MIN_DRAG_SIZE {
                    self.document
                        .update(|document| document.resize_object(id, bounds));
                }
            }
            _ => {}
        }
    }

    fn begin_pan(&self, position: Point) {
        let start_pan = self.controller.get().transform.pan();
        self.controller.get_mut().interaction = Interaction::Panning {
            start_canvas: position,
            start_pan,
        };
    }

    fn update_cursor(&self, context: &mut EventContext<'_>) {
        let cursor = match &self.controller.get().interaction {
            Interaction::Resizing { handle, .. } => match handle {
                super::interaction::ResizeHandle::TopLeft
                | super::interaction::ResizeHandle::BottomRight => CursorIcon::NwseResize,
                super::interaction::ResizeHandle::TopRight
                | super::interaction::ResizeHandle::BottomLeft => CursorIcon::NeswResize,
            },
            Interaction::Panning { .. } => CursorIcon::Pointer,
            _ => CursorIcon::Default,
        };
        context.set_cursor(cursor);
    }
}

impl View for EditorCanvas {
    fn measure(&self, constraints: Constraints, _context: &mut MeasureContext<'_>) -> Size {
        constraints.constrain(Size::ZERO)
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        let document = self.document.get();
        let state = self.controller.get();
        paint_editor_canvas(
            &document,
            state.transform,
            &state.interaction,
            self.active_tool.get(),
            state.selected_node,
            bounds,
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
                if bounds.contains(*position) && *button == PointerButton::Primary =>
            {
                self.handle_primary_press(*position, bounds);
                context.request_redraw_in(bounds);
                self.update_cursor(context);
                EventResult::Consumed
            }
            ViewEvent::PointerPressed { position, button }
                if bounds.contains(*position)
                    && *button == PointerButton::Secondary
                    && self.active_tool.get() == EditorTool::Pen
                    && self.controller.get().active_pen_path.is_some() =>
            {
                self.controller.get_mut().active_pen_path = None;
                context.request_redraw_in(bounds);
                EventResult::Consumed
            }
            ViewEvent::PointerPressed { position, button }
                if bounds.contains(*position)
                    && matches!(button, PointerButton::Middle | PointerButton::Secondary) =>
            {
                self.begin_pan(*position);
                context.request_redraw_in(bounds);
                self.update_cursor(context);
                EventResult::Consumed
            }
            ViewEvent::PointerMoved { position } if self.handle_pointer_move(*position, bounds) => {
                context.request_redraw_in(bounds);
                self.update_cursor(context);
                EventResult::Consumed
            }
            ViewEvent::PointerReleased { button, .. }
                if matches!(
                    button,
                    PointerButton::Primary | PointerButton::Middle | PointerButton::Secondary
                ) && !matches!(self.controller.get().interaction, Interaction::Idle) =>
            {
                self.finish_interaction();
                context.request_redraw_in(bounds);
                self.update_cursor(context);
                EventResult::Consumed
            }
            ViewEvent::Scroll {
                position,
                delta_x,
                delta_y,
            } if bounds.contains(*position) => {
                let mut state = self.controller.get_mut();
                state.transform.zoom_at(*position, bounds, *delta_y);
                let pan = state.transform.pan();
                state.transform.set_pan(Point::new(pan.x - delta_x, pan.y));
                drop(state);
                context.request_redraw_in(bounds);
                EventResult::Consumed
            }
            ViewEvent::Delete | ViewEvent::Backspace => {
                let selected_node = self.controller.get().selected_node;
                if self.active_tool.get() == EditorTool::NodeEdit
                    && let Some((id, node_index)) = selected_node
                {
                    self.document
                        .update(|document| document.remove_path_node(id, node_index));
                    self.controller.get_mut().selected_node = None;
                } else {
                    self.document
                        .update(|document| document.delete_selected_object());
                }
                context.request_redraw_in(bounds);
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
