use crossterm::event::{read, Event as CtEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use editor_core::event_source::{EventSource, InputEvent};
use editor_core::key::Key;
use editor_core::prelude::Size;

/// crossterm 事件源，实现 EventSource trait。
pub struct CrosstermEventSource;

impl EventSource for CrosstermEventSource {
    fn next_event(&mut self) -> Option<InputEvent> {
        match read() {
            Ok(event) => convert_event(event),
            Err(_) => None,
        }
    }
}

/// 将 crossterm Event 转换为平台无关的 InputEvent。
fn convert_event(event: CtEvent) -> Option<InputEvent> {
    match event {
        CtEvent::Key(key_event) if key_event.kind == KeyEventKind::Press => {
            convert_key_event(key_event).map(InputEvent::Key)
        }
        CtEvent::Resize(width_u16, height_u16) =>
        {
            #[allow(clippy::as_conversions)]
            Some(InputEvent::Resize(Size {
                width: width_u16 as usize,
                height: height_u16 as usize,
            }))
        }
        _ => None,
    }
}

/// 将 crossterm KeyEvent 转换为统一 Key 枚举。
fn convert_key_event(event: KeyEvent) -> Option<Key> {
    let KeyEvent {
        code, modifiers, ..
    } = event;
    match (code, modifiers) {
        // Ctrl 组合键 — 特殊处理 Ctrl+[ 等同于 Escape
        (KeyCode::Char(c), KeyModifiers::CONTROL) => match c {
            '[' => Some(Key::Escape),
            _ => Some(Key::Ctrl(c)),
        },
        // Shift 组合键
        (KeyCode::Char(c), KeyModifiers::SHIFT) => Some(Key::Shift(c)),
        // 普通字符
        (KeyCode::Char(c), KeyModifiers::NONE) => Some(Key::Char(c)),
        // Ctrl + 特殊键
        (KeyCode::Backspace, KeyModifiers::CONTROL) => Some(Key::CtrlBackspace),
        (KeyCode::Delete, KeyModifiers::CONTROL) => Some(Key::CtrlDelete),
        // 特殊键（无修饰）
        (KeyCode::Tab, _) => Some(Key::Tab),
        (KeyCode::Enter, _) => Some(Key::Enter),
        (KeyCode::Esc, _) => Some(Key::Escape),
        (KeyCode::Backspace, _) => Some(Key::Backspace),
        (KeyCode::Delete, _) => Some(Key::Delete),
        // 方向键
        (KeyCode::Up, _) => Some(Key::Up),
        (KeyCode::Down, _) => Some(Key::Down),
        (KeyCode::Left, _) => Some(Key::Left),
        (KeyCode::Right, _) => Some(Key::Right),
        // 翻页/首尾
        (KeyCode::PageUp, _) => Some(Key::PageUp),
        (KeyCode::PageDown, _) => Some(Key::PageDown),
        (KeyCode::Home, _) => Some(Key::Home),
        (KeyCode::End, _) => Some(Key::End),
        // F 键
        (KeyCode::F(n), _) if n <= 12 => Some(Key::F(n)),
        _ => None,
    }
}
