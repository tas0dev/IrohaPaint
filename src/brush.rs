use crate::document::{DocumentColor, StrokeCap, StrokeJoin, StrokeStyle};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrushTip {
    Round,
    Ellipse { roundness: f32, angle: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushDefinition {
    pub name: String,
    pub tip: BrushTip,
    pub width: f32,
    pub minimum_width: f32,
    pub smoothing: f32,
    pub streamline: f32,
    pub taper_start: f32,
    pub taper_end: f32,
    pub color: DocumentColor,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
}

impl BrushDefinition {
    pub fn stroke_style(&self) -> StrokeStyle {
        StrokeStyle {
            width: self.width.max(0.1),
            cap: self.cap,
            join: self.join,
            color: self.color,
        }
    }

    pub fn fitting_tolerance(&self, zoom: f32) -> f32 {
        let smoothing = self.smoothing.clamp(0.0, 1.0);
        (0.55 + smoothing * 1.75) / zoom.max(0.01)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushLibrary {
    presets: Vec<BrushDefinition>,
    active: usize,
}

impl BrushLibrary {
    pub fn presets(&self) -> &[BrushDefinition] {
        &self.presets
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &BrushDefinition {
        &self.presets[self.active]
    }

    pub fn select(&mut self, index: usize) {
        if index < self.presets.len() {
            self.active = index;
        }
    }

    pub fn update_active(&mut self, update: impl FnOnce(&mut BrushDefinition)) {
        update(&mut self.presets[self.active]);
        sanitize(&mut self.presets[self.active]);
    }

    pub fn duplicate_active(&mut self) {
        let mut preset = self.active().clone();
        preset.name = format!("{} Copy", preset.name);
        self.presets.push(preset);
        self.active = self.presets.len() - 1;
    }

    pub fn save_active_as(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let mut preset = self.active().clone();
        preset.name = name.to_owned();
        self.presets.push(preset);
        self.active = self.presets.len() - 1;
    }
}

impl Default for BrushLibrary {
    fn default() -> Self {
        Self {
            presets: vec![
                BrushDefinition {
                    tip: BrushTip::Ellipse {
                        roundness: 0.82,
                        angle: -45.0,
                    },
                    ..preset("Clean Inking", 2.5, 0.72, 0.35)
                },
                preset("Smooth Pencil", 1.8, 0.9, 0.15),
                preset("Monoline", 3.0, 0.45, 0.0),
            ],
            active: 0,
        }
    }
}

fn preset(name: &str, width: f32, smoothing: f32, taper: f32) -> BrushDefinition {
    BrushDefinition {
        name: name.to_owned(),
        tip: BrushTip::Round,
        width,
        minimum_width: 0.2,
        smoothing,
        streamline: 0.5,
        taper_start: taper,
        taper_end: taper,
        color: DocumentColor::BLACK,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
    }
}

fn sanitize(brush: &mut BrushDefinition) {
    brush.width = brush.width.clamp(0.1, 256.0);
    brush.minimum_width = brush.minimum_width.clamp(0.0, 1.0);
    brush.smoothing = brush.smoothing.clamp(0.0, 1.0);
    brush.streamline = brush.streamline.clamp(0.0, 1.0);
    brush.taper_start = brush.taper_start.clamp(0.0, 1.0);
    brush.taper_end = brush.taper_end.clamp(0.0, 1.0);
    if let BrushTip::Ellipse { roundness, angle } = &mut brush.tip {
        *roundness = roundness.clamp(0.05, 1.0);
        *angle = angle.rem_euclid(360.0);
    }
}
