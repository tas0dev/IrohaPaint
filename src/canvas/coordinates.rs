use viewkit::prelude::{Point, Rect};

use crate::document::{DocumentPoint, DocumentRect};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform {
    zoom: f32,
    pan: Point,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Point::new(0.0, 0.0),
        }
    }
}

impl CanvasTransform {
    pub fn zoom(self) -> f32 {
        self.zoom
    }

    pub fn pan(self) -> Point {
        self.pan
    }

    pub fn set_pan(&mut self, pan: Point) {
        self.pan = pan;
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

        self.zoom = ((canvas_bounds.size.width / width).min(canvas_bounds.size.height / height)
            * 0.9)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = Point::new(
            (canvas_bounds.size.width - width * self.zoom) / 2.0,
            (canvas_bounds.size.height - height * self.zoom) / 2.0,
        );
        true
    }

    pub fn document_to_canvas(self, point: DocumentPoint, canvas_bounds: Rect) -> Point {
        Point::new(
            canvas_bounds.origin.x + self.pan.x + point.x * self.zoom,
            canvas_bounds.origin.y + self.pan.y + point.y * self.zoom,
        )
    }

    pub fn canvas_to_document(self, point: Point, canvas_bounds: Rect) -> DocumentPoint {
        DocumentPoint::new(
            (point.x - canvas_bounds.origin.x - self.pan.x) / self.zoom,
            (point.y - canvas_bounds.origin.y - self.pan.y) / self.zoom,
        )
    }

    pub fn document_rect_to_canvas(self, rect: DocumentRect, canvas_bounds: Rect) -> Rect {
        let origin = self.document_to_canvas(DocumentPoint::new(rect.x, rect.y), canvas_bounds);
        Rect::new(
            origin.x,
            origin.y,
            rect.width * self.zoom,
            rect.height * self.zoom,
        )
    }

    pub fn zoom_at(&mut self, canvas_point: Point, canvas_bounds: Rect, scroll_delta: f32) {
        let anchor = self.canvas_to_document(canvas_point, canvas_bounds);
        let factor = (scroll_delta * 0.0015).exp();
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);

        self.pan = Point::new(
            canvas_point.x - canvas_bounds.origin.x - anchor.x * self.zoom,
            canvas_point.y - canvas_bounds.origin.y - anchor.y * self.zoom,
        );
    }
}
