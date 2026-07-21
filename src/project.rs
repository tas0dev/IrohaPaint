use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use viewkit::prelude::State;

use crate::document::{Document, ProjectDecodeError};

#[derive(Debug)]
pub enum ProjectError {
    Io(io::Error),
    Invalid(ProjectDecodeError),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "The project file could not be accessed: {error}"),
            Self::Invalid(error) => write!(formatter, "The project could not be opened: {error}"),
        }
    }
}

impl From<io::Error> for ProjectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProjectDecodeError> for ProjectError {
    fn from(error: ProjectDecodeError) -> Self {
        Self::Invalid(error)
    }
}

pub fn save(document: &Document, path: &Path) -> Result<(), ProjectError> {
    fs::write(path, document.to_project_bytes())?;
    Ok(())
}

pub fn open(path: &Path) -> Result<Document, ProjectError> {
    Ok(Document::from_project_bytes(&fs::read(path)?)?)
}

pub fn save_current(
    document: &State<Document>,
    project_path: &State<Option<PathBuf>>,
    save_as: bool,
) -> Result<bool, ProjectError> {
    let path = if !save_as { project_path.get() } else { None };
    let Some(path) = path.or_else(save_path) else {
        return Ok(false);
    };
    save(&document.get(), &path)?;
    document.update(Document::mark_saved);
    project_path.set(Some(path));
    Ok(true)
}

pub fn open_with_dialog() -> Result<Option<(Document, PathBuf)>, ProjectError> {
    let Some(path) = open_path() else {
        return Ok(None);
    };
    Ok(Some((open(&path)?, path)))
}

pub fn prepare_to_replace(
    document: &State<Document>,
    project_path: &State<Option<PathBuf>>,
) -> Result<bool, ProjectError> {
    if !document.get().is_modified() {
        return Ok(true);
    }
    match prompt_unsaved_changes() {
        UnsavedChoice::Save => save_current(document, project_path, false),
        UnsavedChoice::Discard => Ok(true),
        UnsavedChoice::Cancel => Ok(false),
    }
}

pub fn display_name(path: Option<&Path>) -> String {
    path.and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("Untitled.iroha"))
}

#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

#[cfg(target_os = "windows")]
fn prompt_unsaved_changes() -> UnsavedChoice {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDCANCEL, IDNO, IDYES, MB_ICONWARNING, MB_YESNOCANCEL, MessageBoxW,
    };
    let text = wide("Save changes to this project?\0");
    let title = wide("IrohaPaint\0");
    match unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNOCANCEL | MB_ICONWARNING,
        )
    } {
        IDYES => UnsavedChoice::Save,
        IDNO => UnsavedChoice::Discard,
        IDCANCEL => UnsavedChoice::Cancel,
        _ => UnsavedChoice::Cancel,
    }
}

#[cfg(not(target_os = "windows"))]
fn prompt_unsaved_changes() -> UnsavedChoice {
    UnsavedChoice::Cancel
}

#[cfg(target_os = "windows")]
fn save_path() -> Option<PathBuf> {
    choose_path(true)
}

#[cfg(target_os = "windows")]
fn open_path() -> Option<PathBuf> {
    choose_path(false)
}

#[cfg(target_os = "windows")]
fn choose_path(save: bool) -> Option<PathBuf> {
    use std::mem::size_of;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
        OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    const BUFFER_LENGTH: usize = 32_768;
    let mut file = [0_u16; BUFFER_LENGTH];
    if save {
        let initial = "Untitled.iroha".encode_utf16().collect::<Vec<_>>();
        file[..initial.len()].copy_from_slice(&initial);
    }
    let filter = wide("IrohaPaint Project (*.iroha)\0*.iroha\0All Files (*.*)\0*.*\0\0");
    let title = wide(if save {
        "Save IrohaPaint Project\0"
    } else {
        "Open IrohaPaint Project\0"
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
        Flags: OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            },
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
    let accepted = unsafe {
        if save {
            GetSaveFileNameW(&mut options)
        } else {
            GetOpenFileNameW(&mut options)
        }
    } != 0;
    if !accepted {
        return None;
    }
    let length = file.iter().position(|character| *character == 0)?;
    let mut path = PathBuf::from(String::from_utf16_lossy(&file[..length]));
    if save && path.extension().is_none() {
        path.set_extension("iroha");
    }
    Some(path)
}

#[cfg(not(target_os = "windows"))]
fn save_path() -> Option<PathBuf> {
    None
}

#[cfg(not(target_os = "windows"))]
fn open_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}
