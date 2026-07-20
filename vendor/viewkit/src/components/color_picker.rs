use std::cell::RefCell;
use std::rc::Rc;

use crate::draw_command::{DrawCommand, GradientStop};
use crate::event::{EventContext, EventResult, ViewEvent};
use crate::geometry::{Point, Rect, Size};
use crate::platform::PointerButton;
use crate::state::Binding;
use crate::theme::Color;
use crate::view::{Constraints, MeasureContext, PaintContext, View};

const DEFAULT_WIDTH: f32 = 220.0;
const DEFAULT_HEIGHT: f32 = 180.0;
const HUE_HEIGHT: f32 = 20.0;
const SPACING: f32 = 8.0;
const INDICATOR_SIZE: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragTarget {
    Palette,
    Hue,
}

pub struct ColorPicker {
    color: Binding<Color>,
    dragging: Rc<RefCell<Option<DragTarget>>>,
    on_commit: Option<Rc<RefCell<Box<dyn FnMut(Color)>>>>,
}

impl ColorPicker {
    pub fn new(color: Binding<Color>) -> Self {
        Self {
            color,
            dragging: Rc::new(RefCell::new(None)),
            on_commit: None,
        }
    }

    pub fn on_commit(mut self, callback: impl FnMut(Color) + 'static) -> Self {
        self.on_commit = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    fn palette_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            (bounds.size.height - HUE_HEIGHT - SPACING).max(0.0),
        )
    }

    fn hue_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.origin.x,
            bounds.origin.y + (bounds.size.height - HUE_HEIGHT).max(0.0),
            bounds.size.width,
            HUE_HEIGHT.min(bounds.size.height),
        )
    }

    fn update_palette(&self, bounds: Rect, position: Point) {
        let palette = Self::palette_bounds(bounds);
        if palette.is_empty() {
            return;
        }
        let saturation = ((position.x - palette.origin.x) / palette.size.width).clamp(0.0, 1.0);
        let value = (1.0 - (position.y - palette.origin.y) / palette.size.height).clamp(0.0, 1.0);
        let (hue, _, _, _) = rgb_to_hsv(self.color.get());
        self.color
            .set_without_notification(hsv_to_rgb(hue, saturation, value, 255));
    }

    fn update_hue(&self, bounds: Rect, position: Point) {
        let hue_bounds = Self::hue_bounds(bounds);
        if hue_bounds.is_empty() {
            return;
        }
        let hue = ((position.x - hue_bounds.origin.x) / hue_bounds.size.width).clamp(0.0, 1.0);
        let (_, saturation, value, _) = rgb_to_hsv(self.color.get());
        let saturation = if saturation <= f32::EPSILON {
            1.0
        } else {
            saturation
        };
        let value = if value <= f32::EPSILON { 1.0 } else { value };
        self.color
            .set_without_notification(hsv_to_rgb(hue, saturation, value, 255));
    }

    fn commit(&self) {
        self.color.commit();
        if let Some(on_commit) = self.on_commit.as_ref() {
            (on_commit.borrow_mut())(self.color.get());
        }
    }
}

impl View for ColorPicker {
    fn measure(&self, constraints: Constraints, _context: &mut MeasureContext<'_>) -> Size {
        constraints.constrain(Size::new(DEFAULT_WIDTH, DEFAULT_HEIGHT))
    }

    fn paint(&self, bounds: Rect, context: &mut PaintContext<'_>) {
        if bounds.is_empty() {
            return;
        }
        let palette = Self::palette_bounds(bounds);
        let hue_bounds = Self::hue_bounds(bounds);
        let (hue, saturation, value, _) = rgb_to_hsv(self.color.get());

        context.display_list.push(DrawCommand::FillLinearGradient {
            rect: palette,
            start: Point::new(palette.origin.x, palette.origin.y),
            end: Point::new(palette.origin.x + palette.size.width, palette.origin.y),
            stops: vec![
                GradientStop::new(0.0, Color::WHITE),
                GradientStop::new(1.0, hsv_to_rgb(hue, 1.0, 1.0, 255)),
            ],
        });
        context.display_list.push(DrawCommand::FillLinearGradient {
            rect: palette,
            start: Point::new(palette.origin.x, palette.origin.y),
            end: Point::new(palette.origin.x, palette.origin.y + palette.size.height),
            stops: vec![
                GradientStop::new(0.0, Color::TRANSPARENT),
                GradientStop::new(1.0, Color::BLACK),
            ],
        });
        context.display_list.push(DrawCommand::FillLinearGradient {
            rect: hue_bounds,
            start: Point::new(hue_bounds.origin.x, hue_bounds.origin.y),
            end: Point::new(
                hue_bounds.origin.x + hue_bounds.size.width,
                hue_bounds.origin.y,
            ),
            stops: vec![
                GradientStop::new(0.0, Color::rgb(255, 0, 0)),
                GradientStop::new(1.0 / 6.0, Color::rgb(255, 255, 0)),
                GradientStop::new(2.0 / 6.0, Color::rgb(0, 255, 0)),
                GradientStop::new(3.0 / 6.0, Color::rgb(0, 255, 255)),
                GradientStop::new(4.0 / 6.0, Color::rgb(0, 0, 255)),
                GradientStop::new(5.0 / 6.0, Color::rgb(255, 0, 255)),
                GradientStop::new(1.0, Color::rgb(255, 0, 0)),
            ],
        });

        let palette_indicator = Point::new(
            palette.origin.x + palette.size.width * saturation,
            palette.origin.y + palette.size.height * (1.0 - value),
        );
        let indicator = Rect::new(
            palette_indicator.x - INDICATOR_SIZE / 2.0,
            palette_indicator.y - INDICATOR_SIZE / 2.0,
            INDICATOR_SIZE,
            INDICATOR_SIZE,
        );
        context.display_list.push(DrawCommand::StrokeEllipse {
            rect: indicator,
            color: context.theme.colors.background,
            width: 2.0,
        });

        let hue_x = hue_bounds.origin.x + hue_bounds.size.width * hue;
        context.display_list.push(DrawCommand::StrokeRect {
            rect: Rect::new(
                hue_x - 2.0,
                hue_bounds.origin.y,
                4.0,
                hue_bounds.size.height,
            ),
            color: context.theme.colors.background,
            width: 2.0,
        });
        context.display_list.push(DrawCommand::StrokeRect {
            rect: palette,
            color: context.theme.colors.border,
            width: 1.0,
        });
        context.display_list.push(DrawCommand::StrokeRect {
            rect: hue_bounds,
            color: context.theme.colors.border,
            width: 1.0,
        });
    }

    fn handle_event(
        &self,
        bounds: Rect,
        event: &ViewEvent,
        context: &mut EventContext<'_>,
    ) -> EventResult {
        match event {
            ViewEvent::PointerPressed {
                position,
                button: PointerButton::Primary,
            } if Self::palette_bounds(bounds).contains(*position) => {
                *self.dragging.borrow_mut() = Some(DragTarget::Palette);
                self.update_palette(bounds, *position);
                context.request_redraw_in(bounds.expanded(4.0));
                EventResult::Consumed
            }
            ViewEvent::PointerPressed {
                position,
                button: PointerButton::Primary,
            } if Self::hue_bounds(bounds).contains(*position) => {
                *self.dragging.borrow_mut() = Some(DragTarget::Hue);
                self.update_hue(bounds, *position);
                context.request_redraw_in(bounds.expanded(4.0));
                EventResult::Consumed
            }
            ViewEvent::PointerMoved { position } => {
                match *self.dragging.borrow() {
                    Some(DragTarget::Palette) => self.update_palette(bounds, *position),
                    Some(DragTarget::Hue) => self.update_hue(bounds, *position),
                    None => return EventResult::Ignored,
                }
                context.request_redraw_in(bounds.expanded(4.0));
                EventResult::Consumed
            }
            ViewEvent::PointerReleased {
                position,
                button: PointerButton::Primary,
            } => {
                let Some(target) = self.dragging.borrow_mut().take() else {
                    return EventResult::Ignored;
                };
                match target {
                    DragTarget::Palette => self.update_palette(bounds, *position),
                    DragTarget::Hue => self.update_hue(bounds, *position),
                }
                self.commit();
                context.request_redraw_in(bounds.expanded(4.0));
                EventResult::Consumed
            }
            ViewEvent::FocusChanged { focused: false } => {
                if self.dragging.borrow_mut().take().is_some() {
                    self.commit();
                    context.request_redraw_in(bounds.expanded(4.0));
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32, u8) {
    let red = f32::from(color.red) / 255.0;
    let green = f32::from(color.green) / 255.0;
    let blue = f32::from(color.blue) / 255.0;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if maximum == red {
        ((green - blue) / delta).rem_euclid(6.0) / 6.0
    } else if maximum == green {
        ((blue - red) / delta + 2.0) / 6.0
    } else {
        ((red - green) / delta + 4.0) / 6.0
    };
    let saturation = if maximum <= f32::EPSILON {
        0.0
    } else {
        delta / maximum
    };
    (hue, saturation, maximum, color.alpha)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32, alpha: u8) -> Color {
    let hue = hue.rem_euclid(1.0) * 6.0;
    let chroma = value * saturation;
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match hue as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let offset = value - chroma;
    Color::rgba(
        ((red + offset) * 255.0).round() as u8,
        ((green + offset) * 255.0).round() as u8,
        ((blue + offset) * 255.0).round() as u8,
        alpha,
    )
}
