//! Android向けのwinitウィンドウ・ソフトウェア描画バックエンド。

pub use super::desktop::{
    DesktopBackend as AndroidBackend, DesktopBackendError as AndroidBackendError, SoftwareRenderer,
    SoftwareRendererError,
};

pub use winit::platform::android::activity::AndroidApp;
