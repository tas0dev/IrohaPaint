//! プラットフォームから通知されるイベントを定義

use crate::renderer::Viewport;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonState {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub alt: bool,
    pub shortcut: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Escape,
    Space,
    Z,
    Y,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlatformEvent {
    Resumed {
        viewport: Viewport,
    },
    Resized {
        viewport: Viewport,
    },
    ScaleFactorChanged {
        viewport: Viewport,
    },
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    PointerMoved {
        x: f32,
        y: f32,
    },
    PointerButton {
        button: PointerButton,
        state: ButtonState,
    },
    PointerLeft,
    TextInput {
        text: String,
    },
    KeyInput {
        key: KeyCode,
        state: ButtonState,
        modifiers: KeyModifiers,
        repeat: bool,
    },
    ModifiersChanged {
        modifiers: KeyModifiers,
    },

    Backspace,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Delete,

    SelectLeft,
    SelectRight,
    SelectHome,
    SelectEnd,
    SelectAll,

    Focused(bool),
    RedrawRequested,
    CloseRequested,
}
