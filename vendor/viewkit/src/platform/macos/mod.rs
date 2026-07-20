//! macOS向けのwinitウィンドウ・ソフトウェア描画バックエンド

pub use super::desktop::{
    DesktopBackend as MacOsBackend, DesktopBackendError as MacOsBackendError, SoftwareRenderer,
    SoftwareRendererError,
};
