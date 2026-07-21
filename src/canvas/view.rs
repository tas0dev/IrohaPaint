use viewkit::event::{EventContext, EventResult, ViewEvent};
use viewkit::platform::{ButtonState, KeyCode, KeyModifiers, PointerButton};
use viewkit::prelude::{Color, CursorIcon, Point, Rect, Size, State, View};
use viewkit::view::{Constraints, MeasureContext, PaintContext};

use crate::brush::{BrushKind, BrushLibrary};
use crate::document::{
    BezierNode, CanvasSize, Document, DocumentColor, DocumentPoint, DocumentRect, NodeComponent,
    ObjectId, ObjectKind, ObjectStyle, PaintDab,
};
use crate::editor::{EditorTool, EraserMode};

use super::hit_test::{
    fill_object_at, is_first_path_node, object_at, path_node_at, path_segment_at, resize_handle_at,
};
use super::interaction::{
    Interaction, ROTATE_HANDLE_OFFSET, ShapeDraftKind, group_resized_kind, kind_bounds,
    rotated_kind, translated_kind,
};
use super::paint::{NodePresentation, paint_editor_canvas};
use super::raster_stroke::interpolate_dabs;
use super::region_fill;
use super::state::CanvasController;
use super::stroke::{fit_paint_stroke, fit_pencil_stroke};

const HIT_TOLERANCE: f32 = 6.0;
const MIN_DRAG_SIZE: f32 = 2.0;

pub struct EditorCanvas {
    document: State<Document>,
    active_tool: State<EditorTool>,
    controller: CanvasController,
    brushes: State<BrushLibrary>,
    bindings: CanvasBindings,
}

pub struct CanvasBindings {
    pub fill_color: State<Color>,
    pub blob_width: State<f32>,
    pub paint_size: State<f32>,
    pub paint_opacity: State<f32>,
    pub paint_softness: State<f32>,
    pub eraser_mode: State<usize>,
}

impl EditorCanvas {
    pub fn new(
        document: State<Document>,
        active_tool: State<EditorTool>,
        controller: CanvasController,
        brushes: State<BrushLibrary>,
        bindings: CanvasBindings,
    ) -> Self {
        Self {
            document,
            active_tool,
            controller,
            brushes,
            bindings,
        }
    }

    fn handle_primary_press(&self, position: Point, bounds: Rect) {
        let transform = self.controller.get().transform;
        let document_point = transform.canvas_to_document(position, bounds);
        let tolerance = HIT_TOLERANCE / transform.zoom();
        let drawing_bounds = self.drawing_bounds();
        let inside_drawing_bounds =
            drawing_bounds.is_none_or(|drawing_bounds| drawing_bounds.contains(document_point));

        let active_tool = self.active_tool.get();
        let (layer_locked, alpha_locked) = {
            let document = self.document.get();
            (
                document.selected_layer_is_locked(),
                document.selected_layer_is_alpha_locked(),
            )
        };
        if layer_locked && active_tool != EditorTool::Select {
            return;
        }
        if alpha_locked
            && matches!(
                active_tool,
                EditorTool::Rectangle
                    | EditorTool::Ellipse
                    | EditorTool::Pencil
                    | EditorTool::Fill
                    | EditorTool::Eraser
                    | EditorTool::BlobBrush
                    | EditorTool::Pen
            )
        {
            return;
        }
        if !matches!(active_tool, EditorTool::Select | EditorTool::NodeEdit) {
            self.controller.get_mut().selected_objects.clear();
        }
        match active_tool {
            EditorTool::Select => {
                let current_document = self.document.get();
                let (mut selected, modifiers) = {
                    let state = self.controller.get();
                    (state.selected_objects.clone(), state.modifiers)
                };
                selected.retain(|id| current_document.object(*id).is_some());
                let originals = selected
                    .iter()
                    .filter_map(|id| {
                        current_document
                            .object(*id)
                            .map(|object| (*id, object.kind().clone()))
                    })
                    .collect::<Vec<_>>();
                if !layer_locked && let Some(selection_bounds) = kinds_bounds(&originals) {
                    if let Some(handle) =
                        resize_handle_at(selection_bounds, document_point, tolerance)
                    {
                        self.controller.get_mut().interaction = Interaction::ResizingObjects {
                            originals,
                            original_bounds: selection_bounds,
                            anchor: handle.opposite(selection_bounds),
                            current: document_point,
                            handle,
                        };
                        return;
                    }
                    let rotate = DocumentPoint::new(
                        selection_bounds.x + selection_bounds.width * 0.5,
                        selection_bounds.y - ROTATE_HANDLE_OFFSET / transform.zoom().max(0.01),
                    );
                    if point_distance(rotate, document_point) <= tolerance {
                        let center = rect_center(selection_bounds);
                        let angle = angle_from(center, document_point);
                        self.controller.get_mut().interaction = Interaction::RotatingObjects {
                            originals,
                            center,
                            start_angle: angle,
                            current_angle: angle,
                        };
                        return;
                    }
                }
                let hit = object_at(&current_document, document_point, tolerance);
                if modifiers.shift
                    && let Some(id) = hit
                {
                    if let Some(index) = selected.iter().position(|selected| *selected == id) {
                        selected.remove(index);
                    } else {
                        selected.push(id);
                    }
                    let primary = selected.last().copied();
                    drop(current_document);
                    self.document
                        .update(|document| document.select_object(primary));
                    let mut state = self.controller.get_mut();
                    state.selected_objects = selected;
                    state.interaction = Interaction::Idle;
                    return;
                }
                if let Some(id) = hit {
                    if !selected.contains(&id) {
                        selected.clear();
                        selected.push(id);
                    }
                    let originals = selected
                        .iter()
                        .filter_map(|id| {
                            current_document
                                .object(*id)
                                .map(|object| (*id, object.kind().clone()))
                        })
                        .collect::<Vec<_>>();
                    drop(current_document);
                    self.document
                        .update(|document| document.select_object(Some(id)));
                    let mut state = self.controller.get_mut();
                    state.selected_objects = selected;
                    state.interaction = if layer_locked {
                        Interaction::Idle
                    } else {
                        Interaction::MovingObjects {
                            originals,
                            start: document_point,
                            current: document_point,
                        }
                    };
                    return;
                }
                drop(current_document);
                self.controller.get_mut().interaction = Interaction::SelectingObjects {
                    start: document_point,
                    current: document_point,
                    additive: modifiers.shift,
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
                state.selected_objects = hit.into_iter().collect();
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
                let brush = self.brushes.get().active(BrushKind::Line).clone();
                let pressure = self.controller.get().pointer_pressure.unwrap_or(1.0);
                self.controller.get_mut().interaction = Interaction::DrawingPencil {
                    raw_points: vec![document_point],
                    raw_pressures: vec![pressure],
                    preview: fit_pencil_stroke(
                        &[document_point],
                        &[pressure],
                        brush.fitting_tolerance(transform.zoom()),
                    ),
                    brush,
                };
            }
            EditorTool::Paint => {
                if !inside_drawing_bounds {
                    return;
                }
                let size = self.bindings.paint_size.get().max(1.0);
                let dab = PaintDab {
                    center: document_point,
                    radius: size / 2.0,
                    color: self.brushes.get().active(BrushKind::Paint).color,
                    opacity: self.bindings.paint_opacity.get().clamp(0.0, 1.0),
                    softness: self.bindings.paint_softness.get().clamp(0.0, 1.0),
                };
                if !self
                    .document
                    .update(|document| document.begin_paint_stroke(&[dab]))
                {
                    return;
                }
                let mut state = self.controller.get_mut();
                state.paint_dirty = Some(dab.bounds());
                state.interaction = Interaction::Painting {
                    last_input: Some(document_point),
                    distance_since_dab: 0.0,
                    spacing: (size * 0.12).max(0.75 / transform.zoom().max(0.01)),
                    dab,
                };
            }
            EditorTool::Fill => {
                if !inside_drawing_bounds {
                    return;
                }
                let target = {
                    let document = self.document.get();
                    fill_object_at(&document, document_point)
                };
                if let Some((source_layer, id)) = target {
                    let color = self.brushes.get().active(BrushKind::Paint).color;
                    self.document
                        .update(|document| document.fill_from_outline(source_layer, id, color));
                    let mut state = self.controller.get_mut();
                    state.active_pen_path = None;
                    state.selected_nodes.clear();
                    state.hovered_node = None;
                    state.hovered_segment = None;
                } else {
                    let region = {
                        let document = self.document.get();
                        region_fill::region_at(&document, document_point)
                    };
                    if let Some(path) = region {
                        let color = self.brushes.get().active(BrushKind::Paint).color;
                        self.document
                            .update(|document| document.add_fill_region(path, color));
                    }
                }
            }
            EditorTool::Eraser => {
                if !inside_drawing_bounds {
                    return;
                }
                match EraserMode::from_index(self.bindings.eraser_mode.get()) {
                    EraserMode::Partial => {
                        let radius = 8.0 / transform.zoom().max(0.01);
                        let started = self.document.update(|document| {
                            document.begin_erasing_path_sections(&[document_point], radius)
                        });
                        self.controller.get_mut().interaction = Interaction::ErasingPathSections {
                            last: document_point,
                            started,
                            radius,
                        };
                    }
                    EraserMode::Object => {
                        let hit = {
                            let document = self.document.get();
                            object_at(&document, document_point, tolerance)
                        };
                        let started = hit.is_some_and(|id| {
                            self.document
                                .update(|document| document.begin_erasing_objects(&[id]))
                        });
                        self.controller.get_mut().interaction = Interaction::ErasingObjects {
                            last: document_point,
                            started,
                        };
                    }
                }
            }
            EditorTool::BlobBrush => {
                if !inside_drawing_bounds {
                    return;
                }
                let brush = self.brushes.get().active(BrushKind::Paint).clone();
                let width = self.bindings.blob_width.get();
                let style = paint_brush_style(&brush, width);
                let pressure = self.controller.get().pointer_pressure.unwrap_or(1.0);
                self.controller.get_mut().interaction = Interaction::DrawingBlob {
                    raw_points: vec![document_point],
                    raw_pressures: vec![pressure],
                    preview: fit_paint_stroke(
                        &[document_point],
                        &[pressure],
                        brush.fitting_tolerance(transform.zoom()),
                    ),
                    style,
                    smoothing: brush.smoothing,
                    streamline: brush.streamline,
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
        let pointer_canvas = bounds.contains(position).then_some(position);
        let cursor_changed = state.pointer_canvas != pointer_canvas;
        state.pointer_canvas = pointer_canvas;
        let transform = state.transform;
        let document_point = transform.canvas_to_document(position, bounds);
        let constrained_point = drawing_bounds
            .map(|drawing_bounds| clamp_point(document_point, drawing_bounds))
            .unwrap_or(document_point);
        let inside_drawing_bounds =
            drawing_bounds.is_none_or(|drawing_bounds| drawing_bounds.contains(document_point));
        let modifiers = state.modifiers;
        let pointer_pressure = state.pointer_pressure.unwrap_or(1.0);

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
            Interaction::MovingObjects { current, .. }
            | Interaction::ResizingObjects { current, .. }
            | Interaction::SelectingNodes { current, .. }
            | Interaction::SelectingObjects { current, .. } => {
                *current = document_point;
                true
            }
            Interaction::RotatingObjects {
                center,
                start_angle,
                current_angle,
                ..
            } => {
                *current_angle = angle_from(*center, document_point);
                if modifiers.shift {
                    let step = std::f32::consts::FRAC_PI_4 / 3.0;
                    *current_angle =
                        *start_angle + ((*current_angle - *start_angle) / step).round() * step;
                }
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
                raw_pressures,
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
                    raw_pressures.push(pointer_pressure);
                    *preview = fit_pencil_stroke(
                        raw_points,
                        raw_pressures,
                        brush.fitting_tolerance(transform.zoom()),
                    );
                }
                true
            }
            Interaction::ErasingObjects { last, started } => {
                if !inside_drawing_bounds {
                    *last = document_point;
                    return false;
                }
                let tolerance = HIT_TOLERANCE / transform.zoom().max(0.01);
                let ids = {
                    let document = self.document.get();
                    eraser_hits(&document, *last, document_point, tolerance)
                };
                *last = document_point;
                if ids.is_empty() {
                    return false;
                }
                let was_started = *started;
                drop(state);
                let changed = if was_started {
                    self.document
                        .update(|document| document.continue_erasing_objects(&ids))
                } else {
                    self.document
                        .update(|document| document.begin_erasing_objects(&ids))
                };
                if changed
                    && !was_started
                    && let Interaction::ErasingObjects { started, .. } =
                        &mut self.controller.get_mut().interaction
                {
                    *started = true;
                }
                changed
            }
            Interaction::ErasingPathSections {
                last,
                started,
                radius,
            } => {
                if !inside_drawing_bounds {
                    *last = document_point;
                    return false;
                }
                let points = eraser_points(*last, document_point, (*radius * 0.5).max(0.1));
                *last = document_point;
                let was_started = *started;
                let erase_radius = *radius;
                drop(state);
                let changed = if was_started {
                    self.document.update(|document| {
                        document.continue_erasing_path_sections(&points, erase_radius)
                    })
                } else {
                    self.document.update(|document| {
                        document.begin_erasing_path_sections(&points, erase_radius)
                    })
                };
                if changed
                    && !was_started
                    && let Interaction::ErasingPathSections { started, .. } =
                        &mut self.controller.get_mut().interaction
                {
                    *started = true;
                }
                changed
            }
            Interaction::Painting {
                last_input,
                distance_since_dab,
                spacing,
                dab,
            } => {
                if !inside_drawing_bounds {
                    *last_input = None;
                    *distance_since_dab = 0.0;
                    return false;
                }
                let Some(previous) = *last_input else {
                    let resumed_dab = PaintDab {
                        center: document_point,
                        ..*dab
                    };
                    *last_input = Some(document_point);
                    state.paint_dirty = Some(resumed_dab.bounds());
                    drop(state);
                    self.document
                        .update(|document| document.continue_paint_stroke(&[resumed_dab]));
                    return true;
                };
                let dabs =
                    interpolate_dabs(previous, document_point, distance_since_dab, *spacing, *dab);
                *last_input = Some(document_point);
                if dabs.is_empty() {
                    return false;
                }
                state.paint_dirty = paint_dab_bounds(&dabs);
                drop(state);
                self.document
                    .update(|document| document.continue_paint_stroke(&dabs));
                true
            }
            Interaction::DrawingBlob {
                raw_points,
                raw_pressures,
                preview,
                smoothing,
                streamline,
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
                    raw_pressures.push(pointer_pressure);
                    let tolerance = (0.45
                        + smoothing.clamp(0.0, 1.0) * 1.35
                        + streamline.clamp(0.0, 1.0) * 0.9)
                        / transform.zoom().max(0.01);
                    *preview = fit_paint_stroke(raw_points, raw_pressures, tolerance);
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
                    || (self.active_tool.get() == EditorTool::BlobBrush && cursor_changed)
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
                let id = self.document.update(|document| match kind {
                    ShapeDraftKind::Rectangle => document.add_rectangle_with_style(bounds, style),
                    ShapeDraftKind::Ellipse => document.add_ellipse_with_style(bounds, style),
                });
                self.controller.get_mut().selected_objects = vec![id];
            }
            Interaction::DrawingPencil {
                preview: Some(path),
                brush,
                ..
            } => {
                let id = self.document.update(|document| {
                    document.add_variable_width_path(path, brush.stroke_style())
                });
                let mut state = self.controller.get_mut();
                state.selected_objects = vec![id];
                state.selected_nodes.clear();
                state.active_pen_path = None;
            }
            Interaction::DrawingBlob {
                preview: Some(path),
                style,
                ..
            } => {
                let id = self
                    .document
                    .update(|document| document.add_variable_width_path(path, style.stroke));
                let mut state = self.controller.get_mut();
                state.selected_objects = vec![id];
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
                state.selected_objects = vec![id];
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
            Interaction::SelectingObjects {
                start,
                current,
                additive,
            } => {
                let selection = DocumentRect::from_points(start, current);
                let mut selected = if additive {
                    self.controller.get().selected_objects.clone()
                } else {
                    Vec::new()
                };
                if selection.width * zoom >= MIN_DRAG_SIZE
                    || selection.height * zoom >= MIN_DRAG_SIZE
                {
                    let crossing = current.x < start.x;
                    let document = self.document.get();
                    if let Some(layer_index) = document.selected_layer()
                        && let Some(layer) = document.layers().get(layer_index)
                    {
                        for object in layer.objects() {
                            let selected_by_marquee = if crossing {
                                rects_intersect(selection, object.bounds())
                            } else {
                                rect_contains_rect(selection, object.bounds())
                            };
                            if selected_by_marquee && !selected.contains(&object.id()) {
                                selected.push(object.id());
                            }
                        }
                    }
                }
                let primary = selected.last().copied();
                self.document
                    .update(|document| document.select_object(primary));
                self.controller.get_mut().selected_objects = selected;
            }
            Interaction::MovingObjects {
                originals,
                start,
                current,
            } => {
                let delta = DocumentPoint::new(current.x - start.x, current.y - start.y);
                if delta.x.abs() * zoom >= 0.5 || delta.y.abs() * zoom >= 0.5 {
                    let replacements = originals
                        .iter()
                        .map(|(id, kind)| (*id, translated_kind(kind, delta)))
                        .collect::<Vec<_>>();
                    self.document
                        .update(|document| document.replace_object_kinds(&replacements));
                }
            }
            Interaction::ResizingObjects {
                originals,
                original_bounds,
                anchor,
                current,
                ..
            } => {
                let bounds = DocumentRect::from_points(anchor, current);
                if bounds.width * zoom >= MIN_DRAG_SIZE && bounds.height * zoom >= MIN_DRAG_SIZE {
                    let replacements = originals
                        .iter()
                        .map(|(id, kind)| (*id, group_resized_kind(kind, original_bounds, bounds)))
                        .collect::<Vec<_>>();
                    self.document
                        .update(|document| document.replace_object_kinds(&replacements));
                }
            }
            Interaction::RotatingObjects {
                originals,
                center,
                start_angle,
                current_angle,
            } => {
                let angle = current_angle - start_angle;
                if angle.abs() >= 0.001 {
                    let replacements = originals
                        .iter()
                        .map(|(id, kind)| (*id, rotated_kind(kind, center, angle)))
                        .collect::<Vec<_>>();
                    self.document
                        .update(|document| document.replace_object_kinds(&replacements));
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
            stroke: self.brushes.get().active(BrushKind::Line).stroke_style(),
            fill: document_color(self.bindings.fill_color.get()),
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
            Interaction::ResizingObjects { handle, .. } => match handle {
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

    fn take_redraw_bounds(&self, canvas_bounds: Rect) -> Rect {
        let mut state = self.controller.get_mut();
        let Some(mut dirty) = state.paint_dirty.take() else {
            return canvas_bounds;
        };
        let margin = 2.0 / state.transform.zoom().max(0.01);
        dirty.x -= margin;
        dirty.y -= margin;
        dirty.width += margin * 2.0;
        dirty.height += margin * 2.0;
        state
            .transform
            .document_rect_to_canvas(dirty, canvas_bounds)
            .intersection(canvas_bounds)
            .unwrap_or_default()
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
            state.reference_image.as_ref(),
            self.active_tool.get(),
            NodePresentation {
                selected_objects: &state.selected_objects,
                selected: &state.selected_nodes,
                hovered: state.hovered_node,
                segment: state.hovered_segment,
                brush_cursor: (self.active_tool.get() == EditorTool::BlobBrush)
                    .then_some(state.pointer_canvas)
                    .flatten()
                    .map(|position| (position, self.bindings.blob_width.get())),
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
            ViewEvent::PointerPressureChanged { pressure } => {
                self.controller.get_mut().pointer_pressure = Some(*pressure);
                EventResult::Ignored
            }
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
                context.request_redraw_in(self.take_redraw_bounds(bounds));
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
                context.request_redraw_in(self.take_redraw_bounds(bounds));
                self.update_cursor(context);
                EventResult::Consumed
            }
            ViewEvent::PointerLeft => {
                self.controller.get_mut().pointer_canvas = None;
                context.request_redraw_in(bounds);
                EventResult::Ignored
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
                    let selected = self.controller.get().selected_objects.clone();
                    if selected.is_empty() {
                        self.document
                            .update(|document| document.delete_selected_object());
                    } else {
                        self.document
                            .update(|document| self.controller.delete_selection(document));
                    }
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
                    let cancel_change = matches!(state.interaction, Interaction::Painting { .. })
                        || matches!(
                            state.interaction,
                            Interaction::ErasingObjects { started: true, .. }
                                | Interaction::ErasingPathSections { started: true, .. }
                        );
                    state.interaction = Interaction::Idle;
                    state.active_pen_path = None;
                    drop(state);
                    if cancel_change {
                        self.document.update(Document::cancel_in_progress_change);
                    }
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                (KeyCode::Space, key_state) => {
                    let mut state = self.controller.get_mut();
                    state.space_pressed = *key_state == ButtonState::Pressed;
                    let cancel_change = *key_state == ButtonState::Pressed
                        && (matches!(state.interaction, Interaction::Painting { .. })
                            || matches!(
                                state.interaction,
                                Interaction::ErasingObjects { started: true, .. }
                                    | Interaction::ErasingPathSections { started: true, .. }
                            ));
                    if *key_state == ButtonState::Pressed
                        && matches!(
                            state.interaction,
                            Interaction::DrawingPencil { .. }
                                | Interaction::DrawingBlob { .. }
                                | Interaction::Painting { .. }
                                | Interaction::ErasingObjects { .. }
                                | Interaction::ErasingPathSections { .. }
                        )
                    {
                        state.interaction = Interaction::Idle;
                    }
                    drop(state);
                    if cancel_change {
                        self.document.update(Document::cancel_in_progress_change);
                    }
                    EventResult::Consumed
                }
                (KeyCode::C, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    self.controller.copy_selection(&self.document.get());
                    EventResult::Consumed
                }
                (KeyCode::V, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    self.document
                        .update(|document| self.controller.paste(document));
                    self.active_tool.set(EditorTool::Select);
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                (KeyCode::D, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    self.document
                        .update(|document| self.controller.duplicate_selection(document));
                    self.active_tool.set(EditorTool::Select);
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                (KeyCode::Z, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    let in_progress = matches!(
                        self.controller.get().interaction,
                        Interaction::Painting { .. }
                            | Interaction::ErasingObjects { started: true, .. }
                            | Interaction::ErasingPathSections { started: true, .. }
                    );
                    if in_progress {
                        self.document.update(Document::cancel_in_progress_change);
                    } else if modifiers.shift {
                        self.document.update(Document::redo);
                    } else {
                        self.document.update(Document::undo);
                    }
                    let mut canvas = self.controller.get_mut();
                    canvas.selected_objects.clear();
                    canvas.selected_nodes.clear();
                    canvas.hovered_node = None;
                    canvas.hovered_segment = None;
                    canvas.active_pen_path = None;
                    if in_progress {
                        canvas.interaction = Interaction::Idle;
                    }
                    context.request_redraw_in(bounds);
                    EventResult::Consumed
                }
                (KeyCode::Y, ButtonState::Pressed) if modifiers.shortcut && !*repeat => {
                    let in_progress = matches!(
                        self.controller.get().interaction,
                        Interaction::Painting { .. }
                            | Interaction::ErasingObjects { started: true, .. }
                            | Interaction::ErasingPathSections { started: true, .. }
                    );
                    if in_progress {
                        self.document.update(Document::cancel_in_progress_change);
                    } else {
                        self.document.update(Document::redo);
                    }
                    let mut canvas = self.controller.get_mut();
                    canvas.selected_objects.clear();
                    canvas.selected_nodes.clear();
                    canvas.hovered_node = None;
                    canvas.hovered_segment = None;
                    canvas.active_pen_path = None;
                    if in_progress {
                        canvas.interaction = Interaction::Idle;
                    }
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
            ViewEvent::SelectAll if self.active_tool.get() == EditorTool::Select => {
                self.document
                    .update(|document| self.controller.select_all_objects(document));
                context.request_redraw_in(bounds);
                EventResult::Consumed
            }
            ViewEvent::FocusChanged { focused: false } => {
                let mut state = self.controller.get_mut();
                state.space_pressed = false;
                state.modifiers = KeyModifiers::default();
                if matches!(
                    state.interaction,
                    Interaction::Panning { .. }
                        | Interaction::Painting { .. }
                        | Interaction::ErasingObjects { .. }
                        | Interaction::ErasingPathSections { .. }
                ) {
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

fn kinds_bounds(kinds: &[(ObjectId, ObjectKind)]) -> Option<DocumentRect> {
    kinds
        .iter()
        .map(|(_, kind)| kind_bounds(kind))
        .reduce(union_rects)
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

fn rects_intersect(first: DocumentRect, second: DocumentRect) -> bool {
    first.x <= second.x + second.width
        && first.x + first.width >= second.x
        && first.y <= second.y + second.height
        && first.y + first.height >= second.y
}

fn rect_contains_rect(outer: DocumentRect, inner: DocumentRect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn rect_center(bounds: DocumentRect) -> DocumentPoint {
    DocumentPoint::new(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    )
}

fn point_distance(first: DocumentPoint, second: DocumentPoint) -> f32 {
    let x = second.x - first.x;
    let y = second.y - first.y;
    (x * x + y * y).sqrt()
}

fn angle_from(center: DocumentPoint, point: DocumentPoint) -> f32 {
    (point.y - center.y).atan2(point.x - center.x)
}

fn document_color(color: Color) -> DocumentColor {
    DocumentColor::rgba(color.red, color.green, color.blue, color.alpha)
}

fn paint_dab_bounds(dabs: &[PaintDab]) -> Option<DocumentRect> {
    dabs.iter().map(|dab| dab.bounds()).reduce(|first, second| {
        let x = first.x.min(second.x);
        let y = first.y.min(second.y);
        let right = (first.x + first.width).max(second.x + second.width);
        let bottom = (first.y + first.height).max(second.y + second.height);
        DocumentRect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    })
}

fn eraser_hits(
    document: &Document,
    from: DocumentPoint,
    to: DocumentPoint,
    tolerance: f32,
) -> Vec<crate::document::ObjectId> {
    let delta_x = to.x - from.x;
    let delta_y = to.y - from.y;
    let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();
    let steps = (distance / (tolerance * 0.5).max(0.1)).ceil().max(1.0) as usize;
    let mut ids = Vec::new();
    for step in 0..=steps {
        let amount = step as f32 / steps as f32;
        let point = DocumentPoint::new(from.x + delta_x * amount, from.y + delta_y * amount);
        if let Some(id) = object_at(document, point, tolerance)
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    ids
}

fn eraser_points(from: DocumentPoint, to: DocumentPoint, spacing: f32) -> Vec<DocumentPoint> {
    let delta_x = to.x - from.x;
    let delta_y = to.y - from.y;
    let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();
    let steps = (distance / spacing.max(0.1)).ceil().max(1.0) as usize;
    (0..=steps)
        .map(|step| {
            let amount = step as f32 / steps as f32;
            DocumentPoint::new(from.x + delta_x * amount, from.y + delta_y * amount)
        })
        .collect()
}

fn paint_brush_style(brush: &crate::brush::BrushDefinition, width: f32) -> ObjectStyle {
    ObjectStyle {
        stroke: crate::document::StrokeStyle {
            width: width.max(1.0),
            minimum_width: 1.0,
            taper_start: 0.0,
            taper_end: 0.0,
            tip_roundness: 1.0,
            tip_angle: 0.0,
            cap: brush.cap,
            join: brush.join,
            color: brush.color,
        },
        fill: DocumentColor::TRANSPARENT,
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
