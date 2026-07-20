use std::collections::BTreeMap;

use super::{DocumentColor, DocumentPoint, DocumentRect};

pub const PAINT_TILE_SIZE: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintDab {
    pub center: DocumentPoint,
    pub radius: f32,
    pub color: DocumentColor,
    pub opacity: f32,
    pub softness: f32,
}

impl PaintDab {
    pub fn bounds(self) -> DocumentRect {
        let radius = self.radius.max(0.05);
        DocumentRect {
            x: self.center.x - radius,
            y: self.center.y - radius,
            width: radius * 2.0,
            height: radius * 2.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaintTile {
    x: i32,
    y: i32,
    pixels: Vec<u8>,
}

impl PaintTile {
    fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            pixels: vec![0; PAINT_TILE_SIZE as usize * PAINT_TILE_SIZE as usize * 4],
        }
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn y(&self) -> i32 {
        self.y
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn document_bounds(&self) -> DocumentRect {
        DocumentRect {
            x: self.x as f32 * PAINT_TILE_SIZE as f32,
            y: self.y as f32 * PAINT_TILE_SIZE as f32,
            width: PAINT_TILE_SIZE as f32,
            height: PAINT_TILE_SIZE as f32,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaintLayer {
    tiles: BTreeMap<(i32, i32), PaintTile>,
}

impl PaintLayer {
    pub fn tiles(&self) -> impl Iterator<Item = &PaintTile> {
        self.tiles.values()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn bounds(&self) -> Option<DocumentRect> {
        self.tiles
            .values()
            .filter_map(tile_content_bounds)
            .reduce(union)
    }

    pub(super) fn apply_dabs(&mut self, dabs: &[PaintDab], clip: Option<DocumentRect>) {
        for dab in dabs {
            self.apply_dab(*dab, clip);
        }
    }

    fn apply_dab(&mut self, dab: PaintDab, clip: Option<DocumentRect>) {
        let radius = dab.radius.max(0.05);
        let mut left = (dab.center.x - radius).floor() as i32;
        let mut top = (dab.center.y - radius).floor() as i32;
        let mut right = (dab.center.x + radius).ceil() as i32;
        let mut bottom = (dab.center.y + radius).ceil() as i32;
        if let Some(clip) = clip {
            left = left.max(clip.x.floor() as i32);
            top = top.max(clip.y.floor() as i32);
            right = right.min((clip.x + clip.width).ceil() as i32);
            bottom = bottom.min((clip.y + clip.height).ceil() as i32);
        }
        if left >= right || top >= bottom {
            return;
        }

        let tile_size = PAINT_TILE_SIZE as i32;
        for tile_y in top.div_euclid(tile_size)..=(bottom - 1).div_euclid(tile_size) {
            for tile_x in left.div_euclid(tile_size)..=(right - 1).div_euclid(tile_size) {
                let tile = self
                    .tiles
                    .entry((tile_x, tile_y))
                    .or_insert_with(|| PaintTile::new(tile_x, tile_y));
                blend_dab(tile, dab, left, top, right, bottom);
            }
        }
    }
}

fn blend_dab(tile: &mut PaintTile, dab: PaintDab, left: i32, top: i32, right: i32, bottom: i32) {
    let tile_size = PAINT_TILE_SIZE as i32;
    let tile_left = tile.x * tile_size;
    let tile_top = tile.y * tile_size;
    let pixel_left = left.max(tile_left);
    let pixel_top = top.max(tile_top);
    let pixel_right = right.min(tile_left + tile_size);
    let pixel_bottom = bottom.min(tile_top + tile_size);
    let radius = dab.radius.max(0.05);
    let softness = dab.softness.clamp(0.0, 1.0);
    let hard_edge = 1.0 - softness;
    let color_alpha = dab.color.alpha as f32 / 255.0;

    for document_y in pixel_top..pixel_bottom {
        for document_x in pixel_left..pixel_right {
            let dx = document_x as f32 + 0.5 - dab.center.x;
            let dy = document_y as f32 + 0.5 - dab.center.y;
            let distance = (dx * dx + dy * dy).sqrt() / radius;
            if distance >= 1.0 {
                continue;
            }
            let coverage = if softness <= f32::EPSILON || distance <= hard_edge {
                1.0
            } else {
                ((1.0 - distance) / softness).clamp(0.0, 1.0)
            };
            let source_alpha =
                (dab.opacity.clamp(0.0, 1.0) * color_alpha * coverage).clamp(0.0, 1.0);
            if source_alpha <= f32::EPSILON {
                continue;
            }

            let local_x = (document_x - tile_left) as usize;
            let local_y = (document_y - tile_top) as usize;
            let index = (local_y * PAINT_TILE_SIZE as usize + local_x) * 4;
            blend_pixel(&mut tile.pixels[index..index + 4], dab.color, source_alpha);
        }
    }
}

fn blend_pixel(destination: &mut [u8], color: DocumentColor, source_alpha: f32) {
    let destination_alpha = destination[3] as f32 / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        destination.fill(0);
        return;
    }
    for (channel, source) in [color.red, color.green, color.blue].into_iter().enumerate() {
        let destination_color = destination[channel] as f32 / 255.0;
        let source_color = source as f32 / 255.0;
        let output = (source_color * source_alpha
            + destination_color * destination_alpha * (1.0 - source_alpha))
            / output_alpha;
        destination[channel] = (output * 255.0).round() as u8;
    }
    destination[3] = (output_alpha * 255.0).round() as u8;
}

fn tile_content_bounds(tile: &PaintTile) -> Option<DocumentRect> {
    let mut min_x = PAINT_TILE_SIZE;
    let mut min_y = PAINT_TILE_SIZE;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for y in 0..PAINT_TILE_SIZE {
        for x in 0..PAINT_TILE_SIZE {
            let index = (y as usize * PAINT_TILE_SIZE as usize + x as usize) * 4 + 3;
            if tile.pixels[index] == 0 {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
        }
    }
    found.then_some(DocumentRect {
        x: tile.x as f32 * PAINT_TILE_SIZE as f32 + min_x as f32,
        y: tile.y as f32 * PAINT_TILE_SIZE as f32 + min_y as f32,
        width: (max_x - min_x) as f32,
        height: (max_y - min_y) as f32,
    })
}

fn union(first: DocumentRect, second: DocumentRect) -> DocumentRect {
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
}
