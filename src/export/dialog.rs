use std::path::PathBuf;

use super::ExportFormat;

pub struct ExportTarget {
    pub path: PathBuf,
    pub format: ExportFormat,
    pub overwrite_confirmed: bool,
}

#[cfg(target_os = "windows")]
pub fn save_target(format: ExportFormat) -> Option<ExportTarget> {
    use std::mem::size_of;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetSaveFileNameW, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    const BUFFER_LENGTH: usize = 32_768;
    let mut file = [0_u16; BUFFER_LENGTH];
    let initial = "IrohaPaint".encode_utf16().collect::<Vec<_>>();
    file[..initial.len()].copy_from_slice(&initial);
    let filter = wide(match format {
        ExportFormat::Svg => "SVG (*.svg)\0*.svg\0\0",
        ExportFormat::Png { .. } => "PNG (*.png)\0*.png\0\0",
    });
    let title = wide(match format {
        ExportFormat::Svg => "Export SVG\0",
        ExportFormat::Png { scale: 1 } => "Export PNG 1x\0",
        ExportFormat::Png { scale: 2 } => "Export PNG 2x\0",
        ExportFormat::Png { scale: 4 } => "Export PNG 4x\0",
        ExportFormat::Png { .. } => "Export PNG\0",
    });
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
        Flags: OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
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

    // The common dialog borrows all UTF-16 buffers only for the duration of this call.
    let accepted = unsafe { GetSaveFileNameW(&mut options) } != 0;
    if !accepted {
        return None;
    }
    let length = file.iter().position(|character| *character == 0)?;
    let mut path = PathBuf::from(String::from_utf16_lossy(&file[..length]));
    let extension = match format {
        ExportFormat::Svg => "svg",
        ExportFormat::Png { .. } => "png",
    };
    let overwrite_confirmed = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension));
    path.set_extension(extension);
    Some(ExportTarget {
        path,
        format,
        overwrite_confirmed,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn save_target(_format: ExportFormat) -> Option<ExportTarget> {
    None
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}
