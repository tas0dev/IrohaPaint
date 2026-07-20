use std::path::PathBuf;

use viewkit::prelude::ImageData;

use crate::canvas::CanvasController;
use crate::document::{CanvasSize, Document};

pub fn import_with_dialog(canvas: &CanvasController, document: &Document) -> Result<bool, String> {
    let Some(path) = open_image_path() else {
        return Ok(false);
    };
    let image = ImageData::from_path(&path).map_err(|error| error.to_string())?;
    let CanvasSize::Custom { width, height } = document.properties().canvas_size else {
        return Err(String::from(
            "Set a canvas size before importing a reference",
        ));
    };
    canvas.set_reference_image(image, width, height);
    Ok(true)
}

#[cfg(target_os = "windows")]
fn open_image_path() -> Option<PathBuf> {
    use std::mem::size_of;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    const BUFFER_LENGTH: usize = 32_768;
    let mut file = [0_u16; BUFFER_LENGTH];
    let filter = wide(
        "Images (*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif)\0*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif\0All Files (*.*)\0*.*\0\0",
    );
    let title = wide("Import Reference Image\0");
    let mut options = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: null_mut(),
        hInstance: null_mut(),
        lpstrFilter: filter.as_ptr(),
        lpstrCustomFilter: null_mut(),
        nMaxCustFilter: 0,
        nFilterIndex: 1,
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: BUFFER_LENGTH as u32,
        lpstrFileTitle: null_mut(),
        nMaxFileTitle: 0,
        lpstrInitialDir: null(),
        lpstrTitle: title.as_ptr(),
        Flags: OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST,
        nFileOffset: 0,
        nFileExtension: 0,
        lpstrDefExt: null(),
        lCustData: 0,
        lpfnHook: None,
        lpTemplateName: null(),
        pvReserved: null_mut(),
        dwReserved: 0,
        FlagsEx: 0,
    };
    if unsafe { GetOpenFileNameW(&mut options) } == 0 {
        return None;
    }
    let length = file.iter().position(|character| *character == 0)?;
    Some(PathBuf::from(String::from_utf16_lossy(&file[..length])))
}

#[cfg(not(target_os = "windows"))]
fn open_image_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}
