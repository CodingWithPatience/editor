use crate::command::{Command, Edit, Move, System};
use crate::key::Key;
use crate::renderer::CursorStyle;
use once_cell::sync::Lazy;
use regex::Regex;

type CommandType = Command;
type CommandEdit = Edit;
type CommandMove = Move;
type CommandSystem = System;

static PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(w|wq|e|e!)\s+([\\0-9a-zA-Z_.:/\-\p{Han}]+)$")
        .expect("静态正则表达式模式不应失败")
});

#[derive(Debug, Clone)]
pub struct ModeAction {
    pub mode: Mode,
    pub command_list: Option<Vec<CommandType>>,
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    System(CommandSystem),
    NormalPending(PendingKey),
}

impl Mode {
    /// 返回当前模式对应的光标样式（平台无关）。
    pub fn cursor_style(&self) -> CursorStyle {
        match *self {
            Mode::Normal => CursorStyle::BlinkingBlock,
            Mode::NormalPending(_) => CursorStyle::BlinkingUnderScore,
            Mode::Visual | Mode::VisualLine => CursorStyle::BlinkingBlock,
            _ => CursorStyle::BlinkingBar,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PendingKey {
    D,
    Y,
    G,
    R,
    C,
    F,
    CapitalF,
    T,
    CapitalT,
    Lt,
    Gt,
    YiTextObject,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CommandValue {
    Quit,
    ForceQuit,
    Save,
    SaveAs(String),
    SaveAndQuit,
    SaveAsAndQuit(String),
    Open(String),
    ForceOpen(String),
    NoHighlight,
    SetLineNumbers(bool),
}

impl TryFrom<&str> for CommandValue {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim() {
            "q" => Ok(CommandValue::Quit),
            "q!" => Ok(CommandValue::ForceQuit),
            "w" => Ok(CommandValue::Save),
            "wq" => Ok(CommandValue::SaveAndQuit),
            "x" => Ok(CommandValue::SaveAndQuit),
            "noh" | "nohlsearch" => Ok(CommandValue::NoHighlight),
            "set nu" | "set number" => Ok(CommandValue::SetLineNumbers(true)),
            "set nonu" | "set nonumber" => Ok(CommandValue::SetLineNumbers(false)),
            _ => {
                if let Some(caps) = PATTERN.captures(value) {
                    let command_value = &caps[1];
                    let filename = caps[2].to_string();
                    match command_value {
                        "w" => Ok(CommandValue::SaveAs(filename)),
                        "wq" => Ok(CommandValue::SaveAsAndQuit(filename)),
                        "e" => Ok(CommandValue::Open(filename)),
                        "e!" => Ok(CommandValue::ForceOpen(filename)),
                        _ => Err(format!("Invalid command: {value:?}")),
                    }
                } else {
                    Err(format!("Invalid command: {value:?}"))
                }
            }
        }
    }
}

// ============================================================================
// Key → Command 映射 trait 和实现（平台无关）
// ============================================================================

/// 将统一 Key 转换为编辑器命令的 trait。
pub trait FromKey: Sized {
    fn from_key(key: Key) -> Result<Self, String>;
}

/// 将统一 Key 映射为 vi 编辑命令的扩展 trait。
pub trait ComputeModeAction {
    fn compute_mode_action(self, key: Key) -> ModeAction;
    fn try_command_list(key: Key) -> Option<Vec<CommandType>>;
}

impl ComputeModeAction for Mode {
    fn compute_mode_action(self, key: Key) -> ModeAction {
        match self {
            Mode::Insert => match key {
                Key::Escape => ModeAction {
                    mode: Mode::Normal,
                    command_list: None,
                },
                Key::Ctrl('w') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteWordBackward)]),
                },
                Key::Ctrl('u') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToLineStart)]),
                },
                _ => {
                    let command_list = Mode::try_command_list(key);
                    ModeAction {
                        mode: self,
                        command_list,
                    }
                }
            },
            Mode::Normal => match key {
                Key::Char('i') => ModeAction {
                    mode: Mode::Insert,
                    command_list: None,
                },
                Key::Shift('I') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Move(CommandMove::StartOfLine)]),
                },
                Key::Char('a') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Move(CommandMove::Right)]),
                },
                Key::Shift('A') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Move(CommandMove::EndOfLine)]),
                },
                Key::Char('s') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Delete)]),
                },
                Key::Shift('S') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteWholeLine)]),
                },
                Key::Shift('C') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToEnd)]),
                },
                Key::Shift('D') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToEnd)]),
                },
                Key::Char('x') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Delete)]),
                },
                Key::Shift('X') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteBackward)]),
                },
                Key::Char('o') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![
                        CommandType::Move(CommandMove::EndOfLine),
                        CommandType::Edit(CommandEdit::InsertNewline),
                    ]),
                },
                Key::Shift('O') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![
                        CommandType::Move(CommandMove::StartOfLine),
                        CommandType::Edit(CommandEdit::InsertNewline),
                        CommandType::Move(CommandMove::Up),
                    ]),
                },
                Key::Char('v') => ModeAction {
                    mode: Mode::Visual,
                    command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                },
                Key::Shift('V') => ModeAction {
                    mode: Mode::VisualLine,
                    command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                },
                Key::Shift(':') => ModeAction {
                    mode: Mode::Command,
                    command_list: Some(vec![CommandType::System(CommandSystem::CommandMode)]),
                },
                Key::Ctrl('q') => ModeAction {
                    mode: Mode::System(CommandSystem::Quit),
                    command_list: Some(vec![CommandType::System(CommandSystem::Quit)]),
                },
                Key::Ctrl('s') => ModeAction {
                    mode: Mode::System(CommandSystem::Save),
                    command_list: Some(vec![CommandType::System(CommandSystem::Save)]),
                },
                Key::Ctrl('f') => ModeAction {
                    mode: Mode::System(CommandSystem::Search),
                    command_list: Some(vec![CommandType::System(CommandSystem::Search)]),
                },
                Key::Ctrl('r') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Redo)]),
                },
                Key::Ctrl('d') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::HalfPageDown)]),
                },
                Key::Ctrl('u') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::HalfPageUp)]),
                },
                Key::Char('d') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::D),
                    command_list: None,
                },
                Key::Char('y') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::Y),
                    command_list: None,
                },
                Key::Char('g') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::G),
                    command_list: None,
                },
                Key::Char('r') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::R),
                    command_list: None,
                },
                Key::Char('c') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::C),
                    command_list: None,
                },
                Key::Char('t') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::T),
                    command_list: None,
                },
                Key::Char('f') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::F),
                    command_list: None,
                },
                Key::Shift('T') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::CapitalT),
                    command_list: None,
                },
                Key::Shift('F') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::CapitalF),
                    command_list: None,
                },
                Key::Char(';') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindSameDir)]),
                },
                Key::Char(',') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindOppositeDir)]),
                },
                Key::Shift('J') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::JoinLines)]),
                },
                Key::Shift('~') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::ToggleCase)]),
                },
                Key::Char('.') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::RepeatLastEdit)]),
                },
                Key::Shift('<') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::Lt),
                    command_list: None,
                },
                Key::Shift('>') => ModeAction {
                    mode: Mode::NormalPending(PendingKey::Gt),
                    command_list: None,
                },
                Key::Char('/') => ModeAction {
                    mode: Mode::System(CommandSystem::Search),
                    command_list: Some(vec![CommandType::System(CommandSystem::Search)]),
                },
                Key::Shift('?') => ModeAction {
                    mode: Mode::System(CommandSystem::SearchBackward),
                    command_list: Some(vec![CommandType::System(CommandSystem::SearchBackward)]),
                },
                Key::Shift('*') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::System(CommandSystem::SearchWordForward)]),
                },
                Key::Shift('#') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::System(
                        CommandSystem::SearchWordBackward,
                    )]),
                },
                Key::Shift('%') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::MatchBracket)]),
                },
                Key::Char('p') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Paste)]),
                },
                Key::Shift('Y') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::YankLine)]),
                },
                Key::Shift('P') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::PasteAbove)]),
                },
                Key::Char('u') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Undo)]),
                },
                Key::Char('n') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::SearchNext)]),
                },
                Key::Shift('N') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::SearchPrev)]),
                },
                Key::Escape => ModeAction {
                    mode: Mode::Normal,
                    command_list: None,
                },
                _ => {
                    let command_list = CommandMove::from_key(key)
                        .ok()
                        .map(|cmd| vec![CommandType::Move(cmd)]);
                    ModeAction {
                        mode: self,
                        command_list,
                    }
                }
            },
            Mode::NormalPending(pending_key) => match (key, pending_key) {
                (Key::Escape, _) => ModeAction {
                    mode: Mode::Normal,
                    command_list: None,
                },
                (Key::Char('d'), PendingKey::D) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteLine)]),
                },
                (Key::Char('w'), PendingKey::D) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToNextWordStart)]),
                },
                (_, PendingKey::D) => {
                    if let Ok(move_cmd) = CommandMove::from_key(key) {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: Some(vec![CommandType::Edit(
                                CommandEdit::DeleteRangeToMove(move_cmd),
                            )]),
                        }
                    } else {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: None,
                        }
                    }
                }
                (Key::Char('y'), PendingKey::Y) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::YankLine)]),
                },
                (Key::Char('i'), PendingKey::Y) => ModeAction {
                    mode: Mode::NormalPending(PendingKey::YiTextObject),
                    command_list: None,
                },
                (Key::Char('w'), PendingKey::YiTextObject) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::YankInnerWord)]),
                },
                (_, PendingKey::Y) => {
                    if let Ok(move_cmd) = CommandMove::from_key(key) {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: Some(vec![CommandType::Edit(
                                CommandEdit::YankRangeToMove(move_cmd),
                            )]),
                        }
                    } else {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: None,
                        }
                    }
                }
                (Key::Char('c'), PendingKey::C) => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteWholeLine)]),
                },
                (Key::Char('w'), PendingKey::C) => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToNextWordStart)]),
                },
                (_, PendingKey::C) => {
                    if let Ok(move_cmd) = CommandMove::from_key(key) {
                        ModeAction {
                            mode: Mode::Insert,
                            command_list: Some(vec![CommandType::Edit(
                                CommandEdit::ChangeRangeToMove(move_cmd),
                            )]),
                        }
                    } else {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: None,
                        }
                    }
                }
                (Key::Char('g'), PendingKey::G) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Move(CommandMove::FirstLine)]),
                },
                (Key::Char(ch), PendingKey::R) | (Key::Shift(ch), PendingKey::R) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Replace(ch))]),
                },
                (Key::Char(ch), PendingKey::F) | (Key::Shift(ch), PendingKey::F) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Move(CommandMove::FindChar(ch))]),
                },
                (Key::Char(ch), PendingKey::CapitalF) | (Key::Shift(ch), PendingKey::CapitalF) => {
                    ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::FindCharBackward(
                            ch,
                        ))]),
                    }
                }
                (Key::Char(ch), PendingKey::T) | (Key::Shift(ch), PendingKey::T) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Move(CommandMove::TillChar(ch))]),
                },
                (Key::Char(ch), PendingKey::CapitalT) | (Key::Shift(ch), PendingKey::CapitalT) => {
                    ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::TillCharBackward(
                            ch,
                        ))]),
                    }
                }
                (Key::Shift('<'), PendingKey::Lt) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Deindent)]),
                },
                (Key::Shift('>'), PendingKey::Gt) => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::Indent)]),
                },
                _ => ModeAction {
                    mode: Mode::Normal,
                    command_list: None,
                },
            },
            Mode::Visual | Mode::VisualLine => match key {
                Key::Escape => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                },
                Key::Shift(':') => ModeAction {
                    mode: Mode::Command,
                    command_list: Some(vec![CommandType::System(CommandSystem::CommandMode)]),
                },
                Key::Char('v') => {
                    if matches!(self, Mode::VisualLine) {
                        ModeAction {
                            mode: Mode::Visual,
                            command_list: None,
                        }
                    } else {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                        }
                    }
                }
                Key::Shift('V') => ModeAction {
                    mode: Mode::VisualLine,
                    command_list: None,
                },
                Key::Char('d') | Key::Char('x') => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![
                        CommandType::Edit(CommandEdit::DeleteSelection),
                        CommandType::System(CommandSystem::Dismiss),
                    ]),
                },
                Key::Char('y') => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![
                        CommandType::Edit(CommandEdit::YankSelection),
                        CommandType::System(CommandSystem::Dismiss),
                    ]),
                },
                Key::Char(';') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindSameDir)]),
                },
                Key::Char(',') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindOppositeDir)]),
                },
                Key::Char('c') => ModeAction {
                    mode: Mode::Insert,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteSelection)]),
                },
                Key::Shift('~') => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![
                        CommandType::Edit(CommandEdit::ToggleCaseSelection),
                        CommandType::System(CommandSystem::Dismiss),
                    ]),
                },
                Key::Char('p') | Key::Shift('P') => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![
                        CommandType::Edit(CommandEdit::ReplaceWithYank),
                        CommandType::System(CommandSystem::Dismiss),
                    ]),
                },
                Key::Shift('>') => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![
                        CommandType::Edit(CommandEdit::Indent),
                        CommandType::System(CommandSystem::Dismiss),
                    ]),
                },
                Key::Shift('<') => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![
                        CommandType::Edit(CommandEdit::Deindent),
                        CommandType::System(CommandSystem::Dismiss),
                    ]),
                },
                _ => {
                    if let Ok(move_cmd) = CommandMove::from_key(key) {
                        ModeAction {
                            mode: self,
                            command_list: Some(vec![CommandType::Move(move_cmd)]),
                        }
                    } else {
                        ModeAction {
                            mode: self,
                            command_list: None,
                        }
                    }
                }
            },
            Mode::Command => match key {
                Key::Escape => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                },
                Key::Enter => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::Edit(CommandEdit::InsertNewline)]),
                },
                _ => ModeAction {
                    mode: self,
                    command_list: Mode::try_command_list(key),
                },
            },
            Mode::System(
                CommandSystem::Save | CommandSystem::Search | CommandSystem::SearchBackward,
            ) => match key {
                Key::Escape => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                },
                _ => ModeAction {
                    mode: self,
                    command_list: Mode::try_command_list(key),
                },
            },
            Mode::System(CommandSystem::Quit) => match key {
                Key::Escape => ModeAction {
                    mode: Mode::Normal,
                    command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                },
                Key::Ctrl('q') => ModeAction {
                    mode: self,
                    command_list: Some(vec![CommandType::System(CommandSystem::Quit)]),
                },
                _ => ModeAction {
                    mode: self,
                    command_list: None,
                },
            },
            Mode::System(_) => ModeAction {
                mode: self,
                command_list: None,
            },
        }
    }

    fn try_command_list(key: Key) -> Option<Vec<CommandType>> {
        let mut command_list = CommandEdit::from_key(key)
            .ok()
            .map(|command| vec![CommandType::Edit(command)]);
        if command_list.is_none() {
            command_list = CommandMove::from_key(key)
                .ok()
                .map(|command| vec![CommandType::Move(command)]);
        }
        command_list
    }
}

impl FromKey for Edit {
    fn from_key(key: Key) -> Result<Self, String> {
        match key {
            Key::Char(c) | Key::Shift(c) => Ok(Self::Insert(c)),
            Key::Tab => Ok(Self::Insert('\t')),
            Key::Enter => Ok(Self::InsertNewline),
            Key::Backspace => Ok(Self::DeleteBackward),
            Key::CtrlBackspace => Ok(Self::DeleteWordBackward),
            Key::Delete => Ok(Self::Delete),
            Key::CtrlDelete => Ok(Self::DeleteWord),
            _ => Err(format!("Unsupported key {key:?}")),
        }
    }
}

impl FromKey for Move {
    fn from_key(key: Key) -> Result<Self, String> {
        match key {
            Key::Up => Ok(Self::Up),
            Key::Down => Ok(Self::Down),
            Key::Left => Ok(Self::Left),
            Key::Right => Ok(Self::Right),
            Key::PageUp => Ok(Self::PageUp),
            Key::PageDown => Ok(Self::PageDown),
            Key::Home => Ok(Self::StartOfLine),
            Key::End => Ok(Self::EndOfLine),
            Key::Char(' ') => Ok(Self::Right),
            Key::Char('h') => Ok(Self::Left),
            Key::Char('l') => Ok(Self::Right),
            Key::Char('j') => Ok(Self::Down),
            Key::Char('k') => Ok(Self::Up),
            Key::Char('0') => Ok(Self::StartOfLine),
            Key::Char('w') => Ok(Self::NextWordSplitBySymbol),
            Key::Char('e') => Ok(Self::EndOfWordSplitBySymbol),
            Key::Char('b') => Ok(Self::PrevWordSplitBySymbol),
            Key::Shift('^') => Ok(Self::FirstVisibleCharOfLine),
            Key::Shift('$') => Ok(Self::EndOfLine),
            Key::Shift('G') => Ok(Self::LastLine),
            Key::Shift('W') => Ok(Self::NextWordSplitByWhitespace),
            Key::Shift('E') => Ok(Self::EndOfWordSplitByWhitespace),
            Key::Shift('B') => Ok(Self::PrevWordSplitByWhitespace),
            _ => Err(format!("Unsupported key {key:?}")),
        }
    }
}

impl FromKey for System {
    fn from_key(key: Key) -> Result<Self, String> {
        match key {
            Key::Ctrl('q') => Ok(Self::Quit),
            Key::Ctrl('s') => Ok(Self::Save),
            Key::Ctrl('f') => Ok(Self::Search),
            Key::Escape => Ok(Self::Dismiss),
            _ => Err(format!("Unsupported key {key:?}")),
        }
    }
}
