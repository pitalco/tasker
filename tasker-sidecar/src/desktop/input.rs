use anyhow::{anyhow, Result};
use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

/// Cross-platform input simulator using enigo
pub struct InputSimulator {
    enigo: Mutex<Enigo>,
}

impl InputSimulator {
    /// Create a new input simulator
    pub fn new() -> Result<Self> {
        let settings = Settings::default();
        let enigo = Enigo::new(&settings).map_err(|e| anyhow!("Failed to create Enigo: {:?}", e))?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }

    /// Move mouse to absolute screen coordinates
    pub async fn move_to(&self, x: i32, y: i32) -> Result<()> {
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| anyhow!("Failed to move mouse: {:?}", e))?;
        Ok(())
    }

    /// Click at current mouse position
    pub async fn click(&self, x: i32, y: i32) -> Result<()> {
        self.move_to(x, y).await?;
        // Small delay to ensure mouse position is registered
        sleep(Duration::from_millis(10)).await;

        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .button(Button::Left, Direction::Click)
            .map_err(|e| anyhow!("Failed to click: {:?}", e))?;
        Ok(())
    }

    /// Double click at position
    pub async fn double_click(&self, x: i32, y: i32) -> Result<()> {
        self.move_to(x, y).await?;
        sleep(Duration::from_millis(10)).await;

        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .button(Button::Left, Direction::Click)
            .map_err(|e| anyhow!("Failed to click: {:?}", e))?;
        drop(enigo);

        sleep(Duration::from_millis(50)).await;

        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .button(Button::Left, Direction::Click)
            .map_err(|e| anyhow!("Failed to click: {:?}", e))?;
        Ok(())
    }

    /// Right click at position
    pub async fn right_click(&self, x: i32, y: i32) -> Result<()> {
        self.move_to(x, y).await?;
        sleep(Duration::from_millis(10)).await;

        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .button(Button::Right, Direction::Click)
            .map_err(|e| anyhow!("Failed to right click: {:?}", e))?;
        Ok(())
    }

    /// Middle click at position
    pub async fn middle_click(&self, x: i32, y: i32) -> Result<()> {
        self.move_to(x, y).await?;
        sleep(Duration::from_millis(10)).await;

        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .button(Button::Middle, Direction::Click)
            .map_err(|e| anyhow!("Failed to middle click: {:?}", e))?;
        Ok(())
    }

    /// Press and hold mouse button
    pub async fn mouse_down(&self, button: MouseButton) -> Result<()> {
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let btn = match button {
            MouseButton::Left => Button::Left,
            MouseButton::Right => Button::Right,
            MouseButton::Middle => Button::Middle,
        };
        enigo
            .button(btn, Direction::Press)
            .map_err(|e| anyhow!("Failed to press mouse: {:?}", e))?;
        Ok(())
    }

    /// Release mouse button
    pub async fn mouse_up(&self, button: MouseButton) -> Result<()> {
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        let btn = match button {
            MouseButton::Left => Button::Left,
            MouseButton::Right => Button::Right,
            MouseButton::Middle => Button::Middle,
        };
        enigo
            .button(btn, Direction::Release)
            .map_err(|e| anyhow!("Failed to release mouse: {:?}", e))?;
        Ok(())
    }

    /// Scroll at position
    /// delta_y: positive = scroll down, negative = scroll up
    /// delta_x: positive = scroll right, negative = scroll left
    pub async fn scroll(&self, x: i32, y: i32, delta_x: i32, delta_y: i32) -> Result<()> {
        self.move_to(x, y).await?;
        sleep(Duration::from_millis(10)).await;

        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        if delta_y != 0 {
            enigo
                .scroll(delta_y, Axis::Vertical)
                .map_err(|e| anyhow!("Failed to scroll vertically: {:?}", e))?;
        }

        if delta_x != 0 {
            enigo
                .scroll(delta_x, Axis::Horizontal)
                .map_err(|e| anyhow!("Failed to scroll horizontally: {:?}", e))?;
        }

        Ok(())
    }

    /// Type text string
    pub async fn type_text(&self, text: &str) -> Result<()> {
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .text(text)
            .map_err(|e| anyhow!("Failed to type text: {:?}", e))?;
        Ok(())
    }

    /// Send a key combination (e.g., "Ctrl+C", "Alt+Tab", "Shift+Enter")
    pub async fn send_keys(&self, keys: &str) -> Result<()> {
        let parsed = parse_key_combo(keys)?;
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Press modifiers
        for modifier in &parsed.modifiers {
            enigo
                .key(*modifier, Direction::Press)
                .map_err(|e| anyhow!("Failed to press modifier: {:?}", e))?;
        }

        // Press and release the main key
        if let Some(key) = parsed.key {
            enigo
                .key(key, Direction::Click)
                .map_err(|e| anyhow!("Failed to press key: {:?}", e))?;
        }

        // Release modifiers in reverse order
        for modifier in parsed.modifiers.iter().rev() {
            enigo
                .key(*modifier, Direction::Release)
                .map_err(|e| anyhow!("Failed to release modifier: {:?}", e))?;
        }

        Ok(())
    }

    /// Press a single key
    pub async fn key_press(&self, key: &str) -> Result<()> {
        let key = parse_key(key)?;
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .key(key, Direction::Click)
            .map_err(|e| anyhow!("Failed to press key: {:?}", e))?;
        Ok(())
    }

    /// Hold down a key
    pub async fn key_down(&self, key: &str) -> Result<()> {
        let key = parse_key(key)?;
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .key(key, Direction::Press)
            .map_err(|e| anyhow!("Failed to hold key: {:?}", e))?;
        Ok(())
    }

    /// Release a key
    pub async fn key_up(&self, key: &str) -> Result<()> {
        let key = parse_key(key)?;
        let mut enigo = self.enigo.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        enigo
            .key(key, Direction::Release)
            .map_err(|e| anyhow!("Failed to release key: {:?}", e))?;
        Ok(())
    }

    /// Drag from one point to another
    pub async fn drag(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<()> {
        self.move_to(from_x, from_y).await?;
        sleep(Duration::from_millis(50)).await;

        self.mouse_down(MouseButton::Left).await?;
        sleep(Duration::from_millis(50)).await;

        self.move_to(to_x, to_y).await?;
        sleep(Duration::from_millis(50)).await;

        self.mouse_up(MouseButton::Left).await?;
        Ok(())
    }
}

impl Default for InputSimulator {
    fn default() -> Self {
        Self::new().expect("Failed to create InputSimulator")
    }
}

/// Mouse button enum
#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Parsed key combination
struct ParsedKeyCombo {
    modifiers: Vec<Key>,
    key: Option<Key>,
}

/// Parse a key combo string like "Ctrl+Shift+A" into modifiers and main key
fn parse_key_combo(combo: &str) -> Result<ParsedKeyCombo> {
    let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
    let mut modifiers = Vec::new();
    let mut main_key = None;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let lower = part.to_lowercase();

        // Check if it's a modifier
        let modifier = match lower.as_str() {
            "ctrl" | "control" => Some(Key::Control),
            "alt" => Some(Key::Alt),
            "shift" => Some(Key::Shift),
            "cmd" | "command" | "meta" | "win" | "super" => Some(Key::Meta),
            _ => None,
        };

        if let Some(m) = modifier {
            modifiers.push(m);
        } else if is_last {
            // Last part is the main key
            main_key = Some(parse_key(part)?);
        } else {
            // Non-modifier in middle position
            return Err(anyhow!("Invalid key combo: unexpected '{}' in middle", part));
        }
    }

    Ok(ParsedKeyCombo {
        modifiers,
        key: main_key,
    })
}

/// Parse a single key string to enigo Key
fn parse_key(key: &str) -> Result<Key> {
    let lower = key.to_lowercase();
    let key = match lower.as_str() {
        // Letters
        "a" => Key::Unicode('a'),
        "b" => Key::Unicode('b'),
        "c" => Key::Unicode('c'),
        "d" => Key::Unicode('d'),
        "e" => Key::Unicode('e'),
        "f" => Key::Unicode('f'),
        "g" => Key::Unicode('g'),
        "h" => Key::Unicode('h'),
        "i" => Key::Unicode('i'),
        "j" => Key::Unicode('j'),
        "k" => Key::Unicode('k'),
        "l" => Key::Unicode('l'),
        "m" => Key::Unicode('m'),
        "n" => Key::Unicode('n'),
        "o" => Key::Unicode('o'),
        "p" => Key::Unicode('p'),
        "q" => Key::Unicode('q'),
        "r" => Key::Unicode('r'),
        "s" => Key::Unicode('s'),
        "t" => Key::Unicode('t'),
        "u" => Key::Unicode('u'),
        "v" => Key::Unicode('v'),
        "w" => Key::Unicode('w'),
        "x" => Key::Unicode('x'),
        "y" => Key::Unicode('y'),
        "z" => Key::Unicode('z'),

        // Numbers
        "0" => Key::Unicode('0'),
        "1" => Key::Unicode('1'),
        "2" => Key::Unicode('2'),
        "3" => Key::Unicode('3'),
        "4" => Key::Unicode('4'),
        "5" => Key::Unicode('5'),
        "6" => Key::Unicode('6'),
        "7" => Key::Unicode('7'),
        "8" => Key::Unicode('8'),
        "9" => Key::Unicode('9'),

        // Special keys
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "space" | " " => Key::Space,
        "backspace" | "back" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "escape" | "esc" => Key::Escape,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "insert" | "ins" => Key::Insert,

        // Arrow keys
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,

        // Function keys
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,

        // Modifiers (when used as main key)
        "ctrl" | "control" => Key::Control,
        "alt" => Key::Alt,
        "shift" => Key::Shift,
        "cmd" | "command" | "meta" | "win" | "super" => Key::Meta,
        "capslock" | "caps" => Key::CapsLock,

        // Punctuation and symbols
        "minus" | "-" => Key::Unicode('-'),
        "equals" | "=" => Key::Unicode('='),
        "bracketleft" | "[" => Key::Unicode('['),
        "bracketright" | "]" => Key::Unicode(']'),
        "backslash" | "\\" => Key::Unicode('\\'),
        "semicolon" | ";" => Key::Unicode(';'),
        "quote" | "'" => Key::Unicode('\''),
        "comma" | "," => Key::Unicode(','),
        "period" | "." => Key::Unicode('.'),
        "slash" | "/" => Key::Unicode('/'),
        "grave" | "`" => Key::Unicode('`'),

        // Single character fallback
        _ if key.len() == 1 => Key::Unicode(key.chars().next().unwrap()),

        _ => return Err(anyhow!("Unknown key: {}", key)),
    };

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_combo() {
        let combo = parse_key_combo("Ctrl+C").unwrap();
        assert_eq!(combo.modifiers.len(), 1);
        assert!(matches!(combo.key, Some(Key::Unicode('c'))));

        let combo = parse_key_combo("Ctrl+Shift+A").unwrap();
        assert_eq!(combo.modifiers.len(), 2);
        assert!(matches!(combo.key, Some(Key::Unicode('a'))));

        let combo = parse_key_combo("Alt+Tab").unwrap();
        assert_eq!(combo.modifiers.len(), 1);
        assert!(matches!(combo.key, Some(Key::Tab)));
    }

    #[test]
    fn test_parse_key() {
        assert!(matches!(parse_key("Enter").unwrap(), Key::Return));
        assert!(matches!(parse_key("escape").unwrap(), Key::Escape));
        assert!(matches!(parse_key("F1").unwrap(), Key::F1));
        assert!(matches!(parse_key("a").unwrap(), Key::Unicode('a')));
    }
}
