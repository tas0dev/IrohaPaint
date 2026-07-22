use viewkit::prelude::{Point, Rect};

use crate::document::{DocumentPoint, DocumentRect};

pub(crate) const MIN_ZOOM: f32 = 0.1;
pub(crate) const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform {
    zoom: f32,
    pan: Point,
    rotation: f32,
    flipped_horizontal: bool,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Point::new(0.0, 0.0),
            rotation: 0.0,
            flipped_horizontal: false,
        }
    }
}

impl CanvasTransform {
    pub fn zoom(self) -> f32 {
        self.zoom
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    pub fn pan(self) -> Point {
        self.pan
    }

    pub fn rotation(self) -> f32 {
        self.rotation
    }

    pub fn set_rotation(&mut self, rotation: f32) {
        self.rotation = normalize_angle(rotation);
    }

    pub fn set_pan(&mut self, pan: Point) {
        self.pan = pan;
    }

    pub fn is_flipped_horizontal(self) -> bool {
        self.flipped_horizontal
    }

    pub fn toggle_horizontal_flip(&mut self) {
        self.flipped_horizontal = !self.flipped_horizontal;
    }

    pub fn pan_by_canvas_delta(&mut self, delta_x: f32, delta_y: f32) {
        let mut delta = Point::new(delta_x, delta_y);
        if self.flipped_horizontal {
            delta.x = -delta.x;
        }
        delta = rotate_vector(delta, -self.rotation);
        self.pan = Point::new(self.pan.x + delta.x, self.pan.y + delta.y);
    }

    pub fn fit_canvas(&mut self, width: f32, height: f32, canvas_bounds: Rect) -> bool {
        if canvas_bounds.is_empty()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return false;
        }

        let (sin, cos) = self.rotation.sin_cos();
        let rotated_width = width * cos.abs() + height * sin.abs();
        let rotated_height = width * sin.abs() + height * cos.abs();
        self.zoom = ((canvas_bounds.size.width / rotated_width)
            .min(canvas_bounds.size.height / rotated_height)
            * 0.9)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = Point::new(
            (canvas_bounds.size.width - width * self.zoom) / 2.0,
            (canvas_bounds.size.height - height * self.zoom) / 2.0,
        );
        true
    }

    pub fn document_to_canvas(self, point: DocumentPoint, canvas_bounds: Rect) -> Point {
        let base = Point::new(
            canvas_bounds.origin.x + self.pan.x + point.x * self.zoom,
            canvas_bounds.origin.y + self.pan.y + point.y * self.zoom,
        );
        self.apply_view_transform(base, canvas_bounds)
    }

    pub fn canvas_to_document(self, point: Point, canvas_bounds: Rect) -> DocumentPoint {
        let point = self.remove_view_transform(point, canvas_bounds);
        DocumentPoint::new(
            (point.x - canvas_bounds.origin.x - self.pan.x) / self.zoom,
            (point.y - canvas_bounds.origin.y - self.pan.y) / self.zoom,
        )
    }

    pub fn document_rect_to_canvas(self, rect: DocumentRect, canvas_bounds: Rect) -> Rect {
        points_bounds(self.document_rect_corners(rect, canvas_bounds))
    }

    pub fn document_rect_corners(self, rect: DocumentRect, canvas_bounds: Rect) -> [Point; 4] {
        [
            DocumentPoint::new(rect.x, rect.y),
            DocumentPoint::new(rect.x + rect.width, rect.y),
            DocumentPoint::new(rect.x + rect.width, rect.y + rect.height),
            DocumentPoint::new(rect.x, rect.y + rect.height),
        ]
        .map(|point| self.document_to_canvas(point, canvas_bounds))
    }

    pub fn set_zoom_at(&mut self, zoom: f32, canvas_point: Point, canvas_bounds: Rect) {
        let anchor = self.canvas_to_document(canvas_point, canvas_bounds);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.set_anchor_at(anchor, canvas_point, canvas_bounds);
    }

    pub fn set_anchor_at(
        &mut self,
        anchor: DocumentPoint,
        canvas_point: Point,
        canvas_bounds: Rect,
    ) {
        let canvas_point = self.remove_view_transform(canvas_point, canvas_bounds);
        self.pan = Point::new(
            canvas_point.x - canvas_bounds.origin.x - anchor.x * self.zoom,
            canvas_point.y - canvas_bounds.origin.y - anchor.y * self.zoom,
        );
    }

    pub fn center_on(&mut self, point: DocumentPoint, canvas_bounds: Rect) {
        self.pan = Point::new(
            canvas_bounds.size.width * 0.5 - point.x * self.zoom,
            canvas_bounds.size.height * 0.5 - point.y * self.zoom,
        );
    }

    pub fn zoom_at(&mut self, canvas_point: Point, canvas_bounds: Rect, scroll_delta: f32) {
        let factor = (scroll_delta * 0.0015).exp();
        self.set_zoom_at(self.zoom * factor, canvas_point, canvas_bounds);
    }

    fn apply_view_transform(self, point: Point, bounds: Rect) -> Point {
        let center = bounds_center(bounds);
        let mut point = rotate_around(point, center, self.rotation);
        if self.flipped_horizontal {
            point.x = center.x * 2.0 - point.x;
        }
        point
    }

    fn remove_view_transform(self, mut point: Point, bounds: Rect) -> Point {
        let center = bounds_center(bounds);
        if self.flipped_horizontal {
            point.x = center.x * 2.0 - point.x;
        }
        rotate_around(point, center, -self.rotation)
    }
}

fn bounds_center(bounds: Rect) -> Point {
    Point::new(
        bounds.origin.x + bounds.size.width * 0.5,
        bounds.origin.y + bounds.size.height * 0.5,
    )
}

fn rotate_around(point: Point, center: Point, angle: f32) -> Point {
    let rotated = rotate_vector(Point::new(point.x - center.x, point.y - center.y), angle);
    Point::new(center.x + rotated.x, center.y + rotated.y)
}

fn rotate_vector(point: Point, angle: f32) -> Point {
    let (sin, cos) = angle.sin_cos();
    Point::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

fn points_bounds(points: [Point; 4]) -> Rect {
    let left = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let top = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Rect::new(left, top, right - left, bottom - top)
}

fn normalize_angle(angle: f32) -> f32 {
    let full_turn = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(full_turn) - std::f32::consts::PI
}
