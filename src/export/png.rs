use std::path::Path;

use resvg::usvg;
use tiny_skia::{Pixmap, Transform};

use super::ExportError;
use super::svg::ExportedSvg;

const MAX_DIMENSION: u32 = 32_768;
const MAX_PIXELS: u64 = 268_435_456;

pub fn write(path: &Path, svg: &ExportedSvg, scale: u32) -> Result<(), ExportError> {
    let scale = scale.max(1);
    let width = pixel_dimension(svg.width, scale)?;
    let height = pixel_dimension(svg.height, scale)?;
    if width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(ExportError::ImageTooLarge { width, height });
    }

    let tree = usvg::Tree::from_data(svg.source.as_bytes(), &usvg::Options::default())
        .map_err(ExportError::Svg)?;
    let mut pixmap =
        Pixmap::new(width, height).ok_or(ExportError::ImageTooLarge { width, height })?;
    let tree_size = tree.size();
    let transform = Transform::from_scale(
        width as f32 / tree_size.width(),
        height as f32 / tree_size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap
        .save_png(path)
        .map_err(|error| ExportError::Png(error.to_string()))
}

fn pixel_dimension(points: f32, scale: u32) -> Result<u32, ExportError> {
    let pixels = (points * scale as f32).ceil();
    if !pixels.is_finite() || pixels <= 0.0 || pixels > u32::MAX as f32 {
        return Err(ExportError::InvalidDimensions);
    }
    Ok(pixels as u32)
}
