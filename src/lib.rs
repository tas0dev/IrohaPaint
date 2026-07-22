mod app;
mod brush;
mod canvas;
pub mod document;
mod editor;
mod export;
mod icons;
mod project;
mod reference;
mod views;

use app::IrohaPaint;

pub fn run() -> Result<(), viewkit::ViewKitError> {
    viewkit::run::<IrohaPaint>()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: viewkit::platform::android::AndroidApp) {
    if let Err(error) = viewkit::run_android::<IrohaPaint>(app) {
        eprintln!("IrohaPaint failed to start: {error}");
    }
}
