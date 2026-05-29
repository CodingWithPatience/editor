/// 平台无关的按键枚举，各前端将各自的按键事件转换为此统一类型。
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    /// Ctrl + 字符键
    Ctrl(char),
    /// Shift + 字符键（已应用 Shift 效果，如 Key::Shift('A') 表示大写 A）
    Shift(char),
    /// Ctrl + Backspace
    CtrlBackspace,
    /// Ctrl + Delete
    CtrlDelete,
    /// 功能键 F1-F12
    F(u8),
}
