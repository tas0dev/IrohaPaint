use viewkit::event::{EventContext, EventResult, ViewEvent};
use viewkit::platform::{ButtonState, KeyCode, KeyModifiers, PointerButton};
use viewkit::prelude::{Color, CursorIcon, Point, Rect, Size, State, View};
use viewkit::view::{Constraints, MeasureContext, PaintContext};

use crate::brush::BrushLibrary;
use crate::document::{
    BezierNode, CanvasSize, Document, DocumentColor, DocumentPoint, DocumentRect, NodeComponent,
    ObjectKind, ObjectStyle,
};
use crate::editor::EditorTool;

use super::hit_test::{
    is_first_path_node, object_at, path_node_at, path_segment_at, resize_handle_at,
};
use super::interaction::{Interaction, ShapeDraftKind};
use super::paint::{NodePresentation, paint_editor_canvas};
use super::state::CanvasController;
use super::stroke::{fit_blob_stroke, fit_pencil_stroke};

const HIT_TOLERANCE: f32 = 6.0;
const MIN_DRAG_SIZE: f32 = 2.0;

pub struct EditorCanvas {
    document: State<Document>,
    active_tool: State<EditorTool>,
    controller: CanvasController,
    brushes: State<BrushLibrary>,
    fill_color: State<Color>,
    blob_width: State<f32>,
}

impl EditorCanvas {
    pub fn new(
        document: State<Document>,
        active_tool: State<EditorTool>,
        controller: CanvasController,
        brushes: State<BrushLibrary>,
        fill_color: State<Color>,
        blob_width: State<f32>,
    ) -> Self {
        Self {
            document,
            active_tool,
            controller,
            brushes,
            fill_color,
            blob_width,
        }
    }

    fn handle_primary_press(&self, position: Point, bounds: Rect) {
        let transform = self.controller.get().transform;
        let document_point = transform.canvas_to_document(position, bounds);
        let tolerance = HIT_TOLERANCE / transform.zoom();
        let drawing_bounds = self.drawing_bounds();
        let inside_drawing_bounds =
            drawing_bounds.is_none_or(|drawing_bounds| drawing_bounds.contains(document_point));

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
                let state = self.controller.get();
                let selected_nodes = state
                    .selected_nodes
                    .iter()
                    .filter_map(|(object_id, index)| {
                        (Some(*object_id) == selected_id).then_some(*index)
                    })
                    .collect::<Vec<_>>();
                let modifiers = state.modifiers;
                drop(state);

                if let Some(id) = selected_id
                    && let Some(object) = current_document.object(id)
                    && let ObjectKind::Path { path, .. } = object.kind()
                    && let Some(hit) =
                        path_node_at(path, &selected_nodes, document_point, tolerance)
                {
                    let original = object.kind().clone();
                    drop(current_document);
                    let mut state = self.controller.get_mut();
                    let key = (id, hit.index);
                    if hit.component == NodeComponent::Anchor && modifiers.shift {
                        if let Some(position) = state
                            .selected_nodes
                            .iter()
                            .position(|selected| *selected == key)
                        {
                            state.selected_nodes.remove(position);
                            state.interaction = Interaction::Idle;
                            return;
                        }
                        state.selected_nodes.push(key);
                    } else if !state.selected_nodes.contains(&key) {
                        state.selected_nodes.clear();
                        state.selected_nodes.push(key);
                    }
                    let node_indices = state
                        .selected_nodes
                        .iter()
                        .filter_map(|(object_id, index)| (*object_id == id).then_some(*index))
                        .collect();
                    state.interaction = Interaction::EditingPathNode {
                        id,
                        original,
                        node_index: hit.index,
                        node_indices,
                        component: hit.component,
                        start: document_point,
                        current: document_point,
                        independent: modifiers.alt,
                    };
                    return;
                }

                if let Some(id) = selected_id
                    && let Some(object) = current_document.object(id)
                    && let ObjectKind::Path { path, .. } = object.kind()
                    && let Some(hit) = path_segment_at(path, document_point, tolerance)
                {
                    if modifiers.shift {
                        return;
                    }
                    drop(current_document);
                    if let Some(index) = self
                        .document
                        .update(|document| document.insert_path_node(id, hit.start_index, hit.t))
                    {
                        let mut state = self.controller.get_mut();
                        state.selected_nodes.clear();
                        state.selected_nodes.push((id, index));
                        state.hovered_segment = None;
                        state.interaction = Interaction::Idle;
                    }
                    return;
                }

                if let Some(id) = selected_id
                    && current_document
                        .object(id)
                        .is_some_and(|object| matches!(object.kind(), ObjectKind::Path { .. }))
                    && object_at(&current_document, document_point, tolerance).is_none()
                {
                    drop(current_document);
                    let mut state = self.controller.get_mut();
                    if !modifiers.shift {
                        state.selected_nodes.clear();
                    }
                    state.interaction = Interaction::SelectingNodes {
                        id,
                        start: document_point,
                        current: document_point,
                        additive: modifiers.shift,
                    };
                    return;
                }

                let hit = object_at(&current_document, document_point, tolerance);
                drop(current_document);
                self.document.update(|document| document.select_object(hit));
                let mut state = self.controller.get_mut();
                state.selected_nodes.clear();
                state.hovered_segment = None;
                state.interaction = Interaction::Idle;
            }
            EditorTool::Rectangle => {
                if !inside_drawing_bounds {
                    return;
                }
                self.controller.get_mut().interaction = Interaction::DrawingShape {
                    kind: ShapeDraftKind::Rectangle,
                    start: document_point,
                    current: document_point,
                    style: self.active_object_style(),
                };
            }
            EditorTool::Ellipse => {
                if !inside_drawing_bounds {
                    return;
                }
                self.controller.get_mut().interaction = Interaction::DrawingShape {
                    kind: ShapeDraftKind::Ellipse,
                    start: document_point,
                    current: document_point,
                    style: self.active_object_style(),
                };
            }
            EditorTool::Pencil => {
                if !inside_drawing_bounds {
                    return;
                }
                self.controller.get_mut().interaction = Interaction::DrawingPencil {
                    raw_points: vec![document_point],
                    preview: None,
                    brush: self.brushes.get().active().clone(),
                };
            }
            EditorTool::BlobBrush => {
                if !inside_drawing_bounds {
                    return;
                }
                let brush = self.brushes.get().active().clone();
                let width = self.blob_width.get();
                let style = blob_style(brush.color);
                self.controller.get_mut().interaction = Interaction::DrawingBlob {
                    raw_points: vec![document_point],
                    preview: fit_blob_stroke(
                        &[document_point],
                        width,
                        brush.fitting_tolerance(transform.zoom()),
                    ),
                    style,
                    width,
                    smoothing: brush.smoothing,
                };
            }
            EditorTool::Pen => {
                if !inside_drawing_bounds {
                    return;
                }
                let active_path = self.controller.get().active_pen_path;
                let current_document = self.document.get();
                let active_object = active_path.and_then(|id| {
                    current_document
                        .object(id)
                        .map(|object| (id, object.kind().clone()))
                });

                if let Some((id, ObjectKind::Path { path, .. })) = &active_object
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
        let drawing_bounds = self.drawing_bounds();
        let mut state = self.controller.get_mut();
        let transform = state.transform;
        let document_point = transform.canvas_to_document(position, bounds);
        let constrained_point = drawing_bounds
            .map(|drawing_bounds| clamp_point(document_point, drawing_bounds))
            .unwrap_or(document_point);
        let inside_drawing_bounds =
            drawing_bounds.is_none_or(|drawing_bounds| drawing_bounds.contains(document_point));
        let modifiers = state.modifiers;

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
            Interaction::DrawingShape { current, .. } => {
                *current = constrained_point;
                true
            }
            Interaction::Moving { current, .. }
            | Interaction::Resizing { current, .. }
            | Interaction::SelectingNodes { current, .. } => {
                *current = document_point;
                true
            }
            Interaction::EditingPathNode {
                original,
                node_index,
                component,
                current,
                independent,
                ..
            } => {
                *independent = modifiers.alt;
                *current = if modifiers.shift && *component != NodeComponent::Anchor {
                    path_node_position(original, *node_index)
                        .map_or(document_point, |anchor| snap_handle(anchor, document_point))
                } else {
                    document_point
                };
                true
            }
            Interaction::DrawingPencil {
                raw_points,
                preview,
                brush,
            } => {
                if !inside_drawing_bounds {
                    return false;
                }
                let should_add = raw_points.last().is_none_or(|last| {
                    let delta_x = (document_point.x - last.x) * transform.zoom();
                    let delta_y = (document_point.y - last.y) * transform.zoom();
                    delta_x * delta_x + delta_y * delta_y >= 0.75 * 0.75
                });
                if should_add {
                    raw_points.push(document_point);
                    *preview =
                        fit_pencil_stroke(raw_points, brush.fitting_tolerance(transform.zoom()));
                }
                true
            }
            Interaction::DrawingBlob {
                raw_points,
                preview,
                width,
                smoothing,
                ..
            } => {
                if !inside_drawing_bounds {
                    return false;
                }
                let should_add = raw_points.last().is_none_or(|last| {
                    let delta_x = (document_point.x - last.x) * transform.zoom();
                    let delta_y = (document_point.y - last.y) * transform.zoom();
                    delta_x * delta_x + delta_y * delta_y >= 0.75 * 0.75
                });
                if should_add {
                    raw_points.push(document_point);
                    let tolerance =
                        (0.55 + smoothing.clamp(0.0, 1.0) * 1.75) / transform.zoom().max(0.01);
                    *preview = fit_blob_stroke(raw_points, *width, tolerance);
                }
                true
            }
            Interaction::PlacingPathNode { handle_out, .. } => {
                *handle_out = constrained_point;
                true
            }
            Interaction::ClosingPath { .. } => true,
            Interaction::Panning { .. } => unreachable!(),
            Interaction::Idle => {
                drop(state);
                self.update_node_hover(document_point, transform.zoom())
            }
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
                style,
            } => {
                let bounds = DocumentRect::from_points(start, current);
                if bounds.width * zoom < MIN_DRAG_SIZE || bounds.height * zoom < MIN_DRAG_SIZE {
                    return;
                }
                self.document.update(|document| match kind {
                    ShapeDraftKind::Rectangle => {
                        document.add_rectangle_with_style(bounds, style);
                    }
                    ShapeDraftKind::Ellipse => {
                        document.add_ellipse_with_style(bounds, style);
                    }
                });
            }
            Interaction::DrawingPencil {
                preview: Some(path),
                brush,
                ..
            } => {
                self.document
                    .update(|document| document.add_fitted_path(path, brush.stroke_style()));
                let mut state = self.controller.get_mut();
                state.selected_nodes.clear();
                state.active_pen_path = None;
            }
            Interaction::DrawingBlob {
                preview: Some(path),
                style,
                ..
            } => {
                self.document
                    .update(|document| document.add_styled_path(path, style));
                let mut state = self.controller.get_mut();
                state.selected_nodes.clear();
                state.active_pen_path = None;
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
                    let style = self.active_object_style();
                    self.document
                        .update(|document| document.add_path_with_object_style(node, style))
                };
                let node_index = self
                    .document
                    .get()
                    .object(id)
                    .and_then(|object| {
                        let ObjectKind::Path { path, .. } = object.kind() else {
                            return None;
                        };
                        path.nodes().len().checked_sub(1)
                    })
                    .unwrap_or(0);
                let mut state = self.controller.get_mut();
                state.active_pen_path = Some(id);
                state.selected_nodes.clear();
                state.selected_nodes.push((id, node_index));
            }
            Interaction::ClosingPath { id } => {
                self.document.update(|document| document.close_path(id));
                self.controller.get_mut().active_pen_path = None;
            }
            Interaction::EditingPathNode {
                id,
                node_index,
                node_indices,
                component,
                start,
                current,
                independent,
                ..
            } => {
                let delta = DocumentPoint::new(current.x - start.x, current.y - start.y);
                if delta.x.abs() * zoom < 0.5 && delta.y.abs() * zoom < 0.5 {
                    return;
                }
                if component == NodeComponent::Anchor {
                    self.document
                        .update(|document| document.translate_path_nodes(id, &node_indices, delta));
                } else {
                    self.document.update(|document| {
                        document.edit_path_node(id, node_index, component, current, independent)
                    });
                }
            }
            Interaction::SelectingNodes {
                id,
                start,
                current,
                additive,
            } => {
                let selection = DocumentRect::from_points(start, current);
                let indices = self
                    .document
                    .get()
                    .object(id)
                    .and_then(|object| match object.kind() {
                        ObjectKind::Path { path, .. } => Some(
                            path.nodes()
                                .iter()
                                .enumerate()
                                .filter_map(|(index, node)| {
                                    selection.contains(node.position).then_some(index)
                                })
                                .collect::<Vec<_>>(),
                        ),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut state = self.controller.get_mut();
                if !additive {
                    state.selected_nodes.clear();
                }
                for index in indices {
                    if !state.selected_nodes.contains(&(id, index)) {
                        state.selected_nodes.push((id, index));
                    }
                }
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

    fn drawing_bounds(&self) -> Option<DocumentRect> {
        match self.document.get().properties().canvas_size {
            CanvasSize::FitArtwork => None,
            CanvasSize::Custom { width, height } => Some(DocumentRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            }),
        }
    }

    fn active_object_style(&self) -> ObjectStyle {
        ObjectStyle {
            stroke: self.brushes.get().active().stroke_style(),
            fill: document_color(self.fill_color.get()),
        }
    }

    fn initialize_transform(&self, bounds: Rect) {
        if self.controller.get().transform_initialized || bounds.is_empty() {
            return;
        }

        let canvas_size = self.document.get().properties().canvas_size;
        let mut state = self.controller.get_mut();
        state.transform_initialized = match canvas_size {
            CanvasSize::Custom { width, height } => {
                state.transform.fit_canvas(width, height, bounds)
            }
            CanvasSize::FitArtwork => true,
        };
    }

    fn update_node_hover(&self, point: DocumentPoint, zoom: f32) -> bool {
        let previous = self.controller.get().hovered_node;
        let previous_segment = self.controller.get().hovered_segment;
        if self.active_tool.get() != EditorTool::NodeEdit {
            let mut state = self.controller.get_mut();
            state.hovered_node = None;
            state.hovered_segment = None;
            return previous.is_some() || previous_segment.is_some();
        }
        let document = self.document.get();
        let Some(id) = document.selected_object() else {
            let mut state = self.controller.get_mut();
            state.hovered_node = None;
            state.hovered_segment = None;
            return previous.is_some() || previous_segment.is_some();
        };
        let Some(ObjectKind::Path { path, .. }) = document.object(id).map(|object| object.kind())
        else {
            let mut state = self.controller.get_mut();
            state.hovered_node = None;
            state.hovered_segment = None;
            return previous.is_some() || previous_segment.is_some();
        };
        let selected = self
            .controller
            .get()
            .selected_nodes
            .iter()
            .filter_map(|(object_id, index)| (*object_id == id).then_some(*index))
            .collect::<Vec<_>>();
        let hit = path_node_at(path, &selected, point, HIT_TOLERANCE / zoom);
        let hovered = hit.map(|hit| (id, hit.index, hit.component));
        let allow_segment = hovered.is_none() && !self.controller.get().modifiers.shift;
        let hovered_segment = allow_segment
            .then(|| path_segment_at(path, point, HIT_TOLERANCE / zoom))
            .flatten()
            .map(|hit| (id, hit));
        let mut state = self.controller.get_mut();
        state.hovered_node = hovered;
        state.hovered_segment = hovered_segment;
        previous != hovered || previous_segment != hovered_segment
    }

    fn update_cursor(&self, context: &mut EventContext<'_>) {
        let state = self.controller.get();
        let cursor = match &state.interaction {
            Interaction::Resizing { handle, .. } => match handle {
                super::interaction::ResizeHandle::TopLeft
                | super::interaction::ResizeHandle::BottomRight => CursorIcon::NwseResize,
                super::interaction::ResizeHandle::TopRight
                | super::interaction::ResizeHandle::BottomLeft => CursorIcon::NeswResize,
            },
            Interaction::Panning { .. } => CursorIcon::Pointer,
            Interaction::Idle
                if state.hovered_node.is_some() || state.hovered_segment.is_some() =>
            {
                CursorIcon::Pointer
            }
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
        self.initialize_transform(bounds);
        let document = self.document.get();
        let state = self.controller.get();
        paint_editor_canvas(
            &document,
            state.transform,
            &state.interaction,
            self.active_tool.get(),
            NodePresentation {
                selected: &state.selected_nodes,
                hovered: state.hovered_node,
                segment: state.hovered_segment,
            },
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
                if bounds.contains(*position)
                    && *button == PointerButton::Primary
                    && self.controller.get().space_pressed =>
            {
                self.begin_pan(*position);
                context.request_redraw_in(bounds);
                self.update_cursor(context);
                EventResult::Consumed
            }
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
                let selected_nodes = self.controller.get().selected_nodes.clone();
                if self.active_tool.get() == EditorTool::NodeEdit
                    && let Some((id, _)) = selected_nodes.first()
                {
                    let id = *id;
                    let indices = selected_nodes
                        .iter()
                        .filter_map(|(object_id, index)| (*object_id == id).then_some(*index))
                        .collect::<Vec<_>>();
                    self.document
                        .update(|document| document.remove_path_nodes(id, &indices));
                    self.controller.get_mut().selected_nodes.clear();
                } else {
                    self.document
                        .update(|document| document.delete_selected_object());
                }
                context.request_redraw_in(bounds);
                EventResult::Consumed
            }
            ViewEvent::KeyInput {
                key,
                state,
                modifiers,
                repeat,
            } => match (key, state) {
                (KeyCode::Escape, ButtonState::Pressed) => {
                    let mut state = self.controller.get_mut();
                    state.interaction = Interaction::Idle;
                    state.active_pen_path = None;
                    drop(state);
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                (KeyCode::Space, key_state) => {
                    let mut state = self.controller.get_mut();
                    state.space_pressed = *key_state == ButtonState::Pressed;
                    if *key_state == ButtonState::Pressed
                        && matches!(
                            state.interaction,
                            Interaction::DrawingPencil { .. } | Interaction::DrawingBlob { .. }
                        )
                    {
                        state.interaction = Interaction::Idle;
                    }
                    EventResult::Consumed
                }
                (KeyCode::Z, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    if modifiers.shift {
                        self.document.update(Document::redo);
                    } else {
                        self.document.update(Document::undo);
                    }
                    let mut canvas = self.controller.get_mut();
                    canvas.selected_nodes.clear();
                    canvas.hovered_node = None;
                    canvas.hovered_segment = None;
                    canvas.active_pen_path = None;
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                (KeyCode::Y, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    self.document.update(Document::redo);
                    let mut canvas = self.controller.get_mut();
                    canvas.selected_nodes.clear();
                    canvas.hovered_node = None;
                    canvas.hovered_segment = None;
                    canvas.active_pen_path = None;
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            },
            ViewEvent::ModifiersChanged { modifiers } => {
                let mut state = self.controller.get_mut();
                state.modifiers = *modifiers;
                if modifiers.shift {
                    state.hovered_segment = None;
                }
                drop(state);
                context.request_redraw_in(bounds);
                self.update_cursor(context);
                EventResult::Ignored
            }
            ViewEvent::FocusChanged { focused: false } => {
                let mut state = self.controller.get_mut();
                state.space_pressed = false;
                state.modifiers = KeyModifiers::default();
                if matches!(state.interaction, Interaction::Panning { .. }) {
                    state.interaction = Interaction::Idle;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

fn clamp_point(point: DocumentPoint, bounds: DocumentRect) -> DocumentPoint {
    DocumentPoint::new(
        point.x.clamp(bounds.x, bounds.x + bounds.width),
        point.y.clamp(bounds.y, bounds.y + bounds.height),
    )
}

fn document_color(color: Color) -> DocumentColor {
    DocumentColor::rgba(color.red, color.green, color.blue, color.alpha)
}

fn blob_style(color: DocumentColor) -> ObjectStyle {
    ObjectStyle {
        stroke: crate::document::StrokeStyle {
            width: 0.1,
            color: DocumentColor::TRANSPARENT,
            ..crate::document::StrokeStyle::default()
        },
        fill: color,
    }
}

fn path_node_position(kind: &ObjectKind, index: usize) -> Option<DocumentPoint> {
    let ObjectKind::Path { path, .. } = kind else {
        return None;
    };
    path.nodes().get(index).map(|node| node.position)
}

fn snap_handle(anchor: DocumentPoint, point: DocumentPoint) -> DocumentPoint {
    let x = point.x - anchor.x;
    let y = point.y - anchor.y;
    let length = (x * x + y * y).sqrt();
    if length <= f32::EPSILON {
        return point;
    }
    let step = std::f32::consts::FRAC_PI_4;
    let angle = (y.atan2(x) / step).round() * step;
    DocumentPoint::new(
        anchor.x + angle.cos() * length,
        anchor.y + angle.sin() * length,
    )
}
