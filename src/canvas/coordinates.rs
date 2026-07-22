use viewkit::prelude::{Point, Rect};

use crate::document::{DocumentPoint, DocumentRect};

pub(crate) const MIN_ZOOM: f32 = 0.1;
pub(crate) const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform {
    zoom: f32,
    pan: Point,
    flipped_horizontal: bool,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Point::new(0.0, 0.0),
            flipped_horizontal: false,
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

    pub fn is_flipped_horizontal(self) -> bool {
        self.flipped_horizontal
    }

    pub fn toggle_horizontal_flip(&mut self) {
        self.flipped_horizontal = !self.flipped_horizontal;
    }

    pub fn pan_by_canvas_delta(&mut self, delta_x: f32, delta_y: f32) {
        let delta_x = if self.flipped_horizontal {
            -delta_x
        } else {
            delta_x
        };
        self.pan = Point::new(self.pan.x + delta_x, self.pan.y + delta_y);
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
        let local_x = self.pan.x + point.x * self.zoom;
        Point::new(
            canvas_bounds.origin.x
                + if self.flipped_horizontal {
                    canvas_bounds.size.width - local_x
                } else {
                    local_x
                },
            canvas_bounds.origin.y + self.pan.y + point.y * self.zoom,
        )
    }

    pub fn canvas_to_document(self, point: Point, canvas_bounds: Rect) -> DocumentPoint {
        let local_x = point.x - canvas_bounds.origin.x;
        let local_x = if self.flipped_horizontal {
            canvas_bounds.size.width - local_x
        } else {
            local_x
        };
        DocumentPoint::new(
            (local_x - self.pan.x) / self.zoom,
            (point.y - canvas_bounds.origin.y - self.pan.y) / self.zoom,
        )
    }

    pub fn document_rect_to_canvas(self, rect: DocumentRect, canvas_bounds: Rect) -> Rect {
        let first = self.document_to_canvas(DocumentPoint::new(rect.x, rect.y), canvas_bounds);
        let second = self.document_to_canvas(
            DocumentPoint::new(rect.x + rect.width, rect.y + rect.height),
            canvas_bounds,
        );
        Rect::new(
            first.x.min(second.x),
            first.y.min(second.y),
            (second.x - first.x).abs(),
            (second.y - first.y).abs(),
        )
    }

    pub fn set_zoom_at(&mut self, zoom: f32, canvas_point: Point, canvas_bounds: Rect) {
        let anchor = self.canvas_to_document(canvas_point, canvas_bounds);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let local_x = canvas_point.x - canvas_bounds.origin.x;
        self.pan = Point::new(
            if self.flipped_horizontal {
                canvas_bounds.size.width - local_x - anchor.x * self.zoom
            } else {
                local_x - anchor.x * self.zoom
            },
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
}
