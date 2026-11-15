//! Platform-neutral input events.

/// Keys the interface handles itself (text arrives as [`Input::Text`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// A letter key with Ctrl (shortcuts: `'a'`, `'z'`, `'y'`, …), lower case.
    Letter(char),
    /// Function key F1–F12 (`F(5)` starts the slide show).
    F(u8),
}

/// Modifier keys held during an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    /// Shift.
    pub shift: bool,
    /// Ctrl (Command on macOS).
    pub ctrl: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
}

/// An input event in device pixels of the view.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// Committed text (typing, IME result).
    Text(String),
    /// A non-text key.
    Key(Key, Mods),
    /// Vertical wheel scroll in device pixels (positive = towards the end).
    Wheel(f32),
    /// Mouse button pressed: `count` 1 = single, 2 = double, 3 = triple click.
    Press {
        /// Pointer x.
        x: f32,
        /// Pointer y.
        y: f32,
        /// Click count.
        count: u8,
        /// Shift held (extend the selection).
        shift: bool,
    },
    /// Pointer moved with no button held (hover).
    Move {
        /// Pointer x.
        x: f32,
        /// Pointer y.
        y: f32,
    },
    /// Mouse button released.
    Release {
        /// Pointer x.
        x: f32,
        /// Pointer y.
        y: f32,
    },
    /// Pointer moved with the button held.
    Drag {
        /// Pointer x.
        x: f32,
        /// Pointer y.
        y: f32,
    },
    /// The view's size changed (device pixels).
    Resize {
        /// Width.
        width: f32,
        /// Height.
        height: f32,
    },
    /// Device pixels per logical pixel changed (display scale, e.g. 1.25).
    Scale(f32),
    /// Caret blink timer.
    Blink,
    /// Interface timer, about every 100 ms (tooltip delays, animations).
    Tick,
    /// The window was maximised or restored.
    Maximized(bool),
    /// The pointer left the window.
    Leave,
    /// Ctrl + wheel: zoom in (positive) or out by this many wheel notches.
    Zoom(f32),
    /// Text being composed by an input method, shown at the caret (empty: none).
    Preedit(String),
    /// Secondary (right) button released: context menu.
    Context {
        /// Pointer x.
        x: f32,
        /// Pointer y.
        y: f32,
    },
}
