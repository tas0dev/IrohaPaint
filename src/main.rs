mod app;
mod document;
mod editor;
mod views;

use app::IrohaPaint;
use viewkit::prelude::*;

fn main() -> Result<(), ViewKitError> {
    run::<IrohaPaint>()
}
