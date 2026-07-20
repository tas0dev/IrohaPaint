mod app;
mod brush;
mod canvas;
pub mod document;
mod editor;
mod export;
mod icons;
mod views;

use app::IrohaPaint;
use viewkit::prelude::*;

fn main() -> Result<(), ViewKitError> {
    run::<IrohaPaint>()
}
