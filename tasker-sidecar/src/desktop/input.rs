//! Cross-platform mouse and keyboard input using enigo
//!
//! Provides a simple API for simulating user input across Windows, macOS, and Linux.

use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
};
use std::thread;
use std::time::Duration;

/// Input controller for mouse and keyboard simulation
pub struct InputController {
    enigo: Enigo,
}

impl InputController {
    /// Create a new input controller
    pub fn new() -> anyhow::Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to create input controller: {:?}", e))?;
        Ok(Self { enigo })
    }

    // ============ Mouse Operations ============

    /// Move mouse to absolute screen coordinates
    pub fn move_mouse(&mut self, x: i32, y: i32) -> anyhow::Result<()> {
        self.enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| anyhow::anyhow!("Failed to move mouse: {:?}", e))
    }

    /// Click at the current mouse position
    pub fn click(&mut self, button: MouseButton) -> anyhow::Result<()> {
        let btn = button.to_enigo();
        self.enigo
            .button(btn, Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to click: {:?}", e))
    }

    /// Click at specific coordinates
    pub fn click_at(&mut self, x: i32, y: i32, button: MouseButton) -> anyhow::Result<()> {
        self.move_mouse(x, y)?;
        thread::sleep(Duration::from_millis(50)); // Small delay for reliability
        self.click(button)
    }

    /// Double-click at the current mouse position
    pub fn double_click(&mut self, button: MouseButton) -> anyhow::Result<()> {
        let btn = button.to_enigo();
        self.enigo
            .button(btn, Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to double-click: {:?}", e))?;
        thread::sleep(Duration::from_millis(50));
        self.enigo
            .button(btn, Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to double-click: {:?}", e))
    }

    /// Double-click at specific coordinates
    pub fn double_click_at(&mut self, x: i32, y: i32, button: MouseButton) -> anyhow::Result<()> {
        self.move_mouse(x, y)?;
        thread::sleep(Duration::from_millis(50));
        self.double_click(button)
    }

    /// Press and hold a mouse button
    pub fn mouse_down(&mut self, button: MouseButton) -> anyhow::Result<()> {
        self.enigo
            .button(button.to_enigo(), Direction::Press)
            .map_err(|e| anyhow::anyhow!("Failed to press mouse button: {:?}", e))
    }

    /// Release a mouse button
    pub fn mouse_up(&mut self, button: MouseButton) -> anyhow::Result<()> {
        self.enigo
            .button(button.to_enigo(), Direction::Release)
            .map_err(|e| anyhow::anyhow!("Failed to release mouse button: {:?}", e))
    }

    /// Scroll the mouse wheel
    pub fn scroll(&mut self, dx: i32, dy: i32) -> anyhow::Result<()> {
        if dx != 0 {
            self.enigo
                .scroll(dx, Axis::Horizontal)
                .map_err(|e| anyhow::anyhow!("Failed to scroll horizontal: {:?}", e))?;
        }
        if dy != 0 {
            self.enigo
                .scroll(dy, Axis::Vertical)
                .map_err(|e| anyhow::anyhow!("Failed to scroll vertical: {:?}", e))?;
        }
        Ok(())
    }

    /// Drag from one point to another
    pub fn drag(&mut self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> anyhow::Result<()> {
        self.move_mouse(from_x, from_y)?;
        thread::sleep(Duration::from_millis(50));
        self.mouse_down(MouseButton::Left)?;
        thread::sleep(Duration::from_millis(50));
        self.move_mouse(to_x, to_y)?;
        thread::sleep(Duration::from_millis(50));
        self.mouse_up(MouseButton::Left)
    }

    // ============ Keyboard Operations ============

    /// Type text string
    pub fn type_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to type text: {:?}", e))
    }

    /// Press a single key
    pub fn key_press(&mut self, key: KeyCode) -> anyhow::Result<()> {
        self.enigo
            .key(key.to_enigo(), Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to press key: {:?}", e))
    }

    /// Hold down a key
    pub fn key_down(&mut self, key: KeyCode) -> anyhow::Result<()> {
        self.enigo
            .key(key.to_enigo(), Direction::Press)
            .map_err(|e| anyhow::anyhow!("Failed to press key down: {:?}", e))
    }

    /// Release a key
    pub fn key_up(&mut self, key: KeyCode) -> anyhow::Result<()> {
        self.enigo
            .key(key.to_enigo(), Direction::Release)
            .map_err(|e| anyhow::anyhow!("Failed to release key: {:?}", e))
    }

    /// Execute a hotkey combination (e.g., Ctrl+C, Alt+Tab)
    pub fn hotkey(&mut self, modifiers: &[Modifier], key: KeyCode) -> anyhow::Result<()> {
        // Press all modifiers
        for modifier in modifiers {
            self.key_down(modifier.to_key_code())?;
        }

        thread::sleep(Duration::from_millis(20));

        // Press the main key
        self.key_press(key)?;

        thread::sleep(Duration::from_millis(20));

        // Release all modifiers in reverse order
        for modifier in modifiers.iter().rev() {
            self.key_up(modifier.to_key_code())?;
        }

        Ok(())
    }
}

impl Default for InputController {
    fn default() -> Self {
        Self::new().expect("Failed to create default InputController")
    }
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    fn to_enigo(self) -> Button {
        match self {
            MouseButton::Left => Button::Left,
            MouseButton::Right => Button::Right,
            MouseButton::Middle => Button::Middle,
        }
    }
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Control,
    Alt,
    Shift,
    Meta, // Windows key / Command key
}

impl Modifier {
    fn to_key_code(self) -> KeyCode {
        match self {
            Modifier::Control => KeyCode::Control,
            Modifier::Alt => KeyCode::Alt,
            Modifier::Shift => KeyCode::Shift,
            Modifier::Meta => KeyCode::Meta,
        }
    }
}

/// Common key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // Numbers
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,

    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // Modifiers
    Control, Alt, Shift, Meta,

    // Navigation
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,

    // Editing
    Backspace, Delete, Enter, Tab, Escape, Space,

    // Special
    Insert, PrintScreen, ScrollLock, Pause,
    CapsLock, NumLock,
}

impl KeyCode {
    fn to_enigo(self) -> Key {
        match self {
            // Letters
            KeyCode::A => Key::Unicode('a'),
            KeyCode::B => Key::Unicode('b'),
            KeyCode::C => Key::Unicode('c'),
            KeyCode::D => Key::Unicode('d'),
            KeyCode::E => Key::Unicode('e'),
            KeyCode::F => Key::Unicode('f'),
            KeyCode::G => Key::Unicode('g'),
            KeyCode::H => Key::Unicode('h'),
            KeyCode::I => Key::Unicode('i'),
            KeyCode::J => Key::Unicode('j'),
            KeyCode::K => Key::Unicode('k'),
            KeyCode::L => Key::Unicode('l'),
            KeyCode::M => Key::Unicode('m'),
            KeyCode::N => Key::Unicode('n'),
            KeyCode::O => Key::Unicode('o'),
            KeyCode::P => Key::Unicode('p'),
            KeyCode::Q => Key::Unicode('q'),
            KeyCode::R => Key::Unicode('r'),
            KeyCode::S => Key::Unicode('s'),
            KeyCode::T => Key::Unicode('t'),
            KeyCode::U => Key::Unicode('u'),
            KeyCode::V => Key::Unicode('v'),
            KeyCode::W => Key::Unicode('w'),
            KeyCode::X => Key::Unicode('x'),
            KeyCode::Y => Key::Unicode('y'),
            KeyCode::Z => Key::Unicode('z'),

            // Numbers
            KeyCode::Num0 => Key::Unicode('0'),
            KeyCode::Num1 => Key::Unicode('1'),
            KeyCode::Num2 => Key::Unicode('2'),
            KeyCode::Num3 => Key::Unicode('3'),
            KeyCode::Num4 => Key::Unicode('4'),
            KeyCode::Num5 => Key::Unicode('5'),
            KeyCode::Num6 => Key::Unicode('6'),
            KeyCode::Num7 => Key::Unicode('7'),
            KeyCode::Num8 => Key::Unicode('8'),
            KeyCode::Num9 => Key::Unicode('9'),

            // Function keys
            KeyCode::F1 => Key::F1,
            KeyCode::F2 => Key::F2,
            KeyCode::F3 => Key::F3,
            KeyCode::F4 => Key::F4,
            KeyCode::F5 => Key::F5,
            KeyCode::F6 => Key::F6,
            KeyCode::F7 => Key::F7,
            KeyCode::F8 => Key::F8,
            KeyCode::F9 => Key::F9,
            KeyCode::F10 => Key::F10,
            KeyCode::F11 => Key::F11,
            KeyCode::F12 => Key::F12,

            // Modifiers
            KeyCode::Control => Key::Control,
            KeyCode::Alt => Key::Alt,
            KeyCode::Shift => Key::Shift,
            KeyCode::Meta => Key::Meta,

            // Navigation
            KeyCode::Up => Key::UpArrow,
            KeyCode::Down => Key::DownArrow,
            KeyCode::Left => Key::LeftArrow,
            KeyCode::Right => Key::RightArrow,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,

            // Editing
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Enter => Key::Return,
            KeyCode::Tab => Key::Tab,
            KeyCode::Escape => Key::Escape,
            KeyCode::Space => Key::Space,

            // Special
            KeyCode::Insert => Key::Insert,
            KeyCode::PrintScreen => Key::Print,
            KeyCode::ScrollLock => Key::Other(0x91), // VK_SCROLL on Windows
            KeyCode::Pause => Key::Pause,
            KeyCode::CapsLock => Key::CapsLock,
            KeyCode::NumLock => Key::Numlock,
        }
    }

    /// Parse a key from a string (for tool input)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "a" => Some(KeyCode::A),
            "b" => Some(KeyCode::B),
            "c" => Some(KeyCode::C),
            "d" => Some(KeyCode::D),
            "e" => Some(KeyCode::E),
            "f" => Some(KeyCode::F),
            "g" => Some(KeyCode::G),
            "h" => Some(KeyCode::H),
            "i" => Some(KeyCode::I),
            "j" => Some(KeyCode::J),
            "k" => Some(KeyCode::K),
            "l" => Some(KeyCode::L),
            "m" => Some(KeyCode::M),
            "n" => Some(KeyCode::N),
            "o" => Some(KeyCode::O),
            "p" => Some(KeyCode::P),
            "q" => Some(KeyCode::Q),
            "r" => Some(KeyCode::R),
            "s" => Some(KeyCode::S),
            "t" => Some(KeyCode::T),
            "u" => Some(KeyCode::U),
            "v" => Some(KeyCode::V),
            "w" => Some(KeyCode::W),
            "x" => Some(KeyCode::X),
            "y" => Some(KeyCode::Y),
            "z" => Some(KeyCode::Z),
            "0" => Some(KeyCode::Num0),
            "1" => Some(KeyCode::Num1),
            "2" => Some(KeyCode::Num2),
            "3" => Some(KeyCode::Num3),
            "4" => Some(KeyCode::Num4),
            "5" => Some(KeyCode::Num5),
            "6" => Some(KeyCode::Num6),
            "7" => Some(KeyCode::Num7),
            "8" => Some(KeyCode::Num8),
            "9" => Some(KeyCode::Num9),
            "f1" => Some(KeyCode::F1),
            "f2" => Some(KeyCode::F2),
            "f3" => Some(KeyCode::F3),
            "f4" => Some(KeyCode::F4),
            "f5" => Some(KeyCode::F5),
            "f6" => Some(KeyCode::F6),
            "f7" => Some(KeyCode::F7),
            "f8" => Some(KeyCode::F8),
            "f9" => Some(KeyCode::F9),
            "f10" => Some(KeyCode::F10),
            "f11" => Some(KeyCode::F11),
            "f12" => Some(KeyCode::F12),
            "ctrl" | "control" => Some(KeyCode::Control),
            "alt" => Some(KeyCode::Alt),
            "shift" => Some(KeyCode::Shift),
            "meta" | "win" | "cmd" | "command" => Some(KeyCode::Meta),
            "up" => Some(KeyCode::Up),
            "down" => Some(KeyCode::Down),
            "left" => Some(KeyCode::Left),
            "right" => Some(KeyCode::Right),
            "home" => Some(KeyCode::Home),
            "end" => Some(KeyCode::End),
            "pageup" | "pgup" => Some(KeyCode::PageUp),
            "pagedown" | "pgdn" => Some(KeyCode::PageDown),
            "backspace" | "bs" => Some(KeyCode::Backspace),
            "delete" | "del" => Some(KeyCode::Delete),
            "enter" | "return" => Some(KeyCode::Enter),
            "tab" => Some(KeyCode::Tab),
            "escape" | "esc" => Some(KeyCode::Escape),
            "space" => Some(KeyCode::Space),
            "insert" | "ins" => Some(KeyCode::Insert),
            "printscreen" | "prtsc" => Some(KeyCode::PrintScreen),
            "scrolllock" => Some(KeyCode::ScrollLock),
            "pause" => Some(KeyCode::Pause),
            "capslock" => Some(KeyCode::CapsLock),
            "numlock" => Some(KeyCode::NumLock),
            _ => None,
        }
    }
}

impl Modifier {
    /// Parse a modifier from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ctrl" | "control" => Some(Modifier::Control),
            "alt" => Some(Modifier::Alt),
            "shift" => Some(Modifier::Shift),
            "meta" | "win" | "cmd" | "command" => Some(Modifier::Meta),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_code_parsing() {
        assert_eq!(KeyCode::from_str("a"), Some(KeyCode::A));
        assert_eq!(KeyCode::from_str("CTRL"), Some(KeyCode::Control));
        assert_eq!(KeyCode::from_str("enter"), Some(KeyCode::Enter));
        assert_eq!(KeyCode::from_str("unknown"), None);
    }

    #[test]
    fn test_modifier_parsing() {
        assert_eq!(Modifier::from_str("ctrl"), Some(Modifier::Control));
        assert_eq!(Modifier::from_str("ALT"), Some(Modifier::Alt));
        assert_eq!(Modifier::from_str("cmd"), Some(Modifier::Meta));
    }
}
