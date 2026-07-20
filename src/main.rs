mod app;
mod canvas;
pub mod document;
mod editor;
mod icons;
mod views;

use app::IrohaPaint;
use viewkit::prelude::*;

fn main() -> Result<(), ViewKitError> {
    run::<IrohaPaint>()
}
