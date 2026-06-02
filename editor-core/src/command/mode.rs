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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Edit, Move, System};
    use crate::key::Key;

    // --- Normal 模式按键映射 ---

    #[test]
    fn normal_i_enters_insert() {
        let action = Mode::Normal.compute_mode_action(Key::Char('i'));
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_none());
    }

    #[test]
    fn normal_a_enters_insert() {
        let action = Mode::Normal.compute_mode_action(Key::Char('a'));
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn normal_esc_stays_normal() {
        let action = Mode::Normal.compute_mode_action(Key::Escape);
        assert_eq!(action.mode, Mode::Normal);
    }

    #[test]
    fn normal_o_enters_insert() {
        let action = Mode::Normal.compute_mode_action(Key::Char('o'));
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn normal_v_enters_visual() {
        let action = Mode::Normal.compute_mode_action(Key::Char('v'));
        assert_eq!(action.mode, Mode::Visual);
    }

    #[test]
    fn normal_shift_v_enters_visual_line() {
        let action = Mode::Normal.compute_mode_action(Key::Shift('V'));
        assert_eq!(action.mode, Mode::VisualLine);
    }

    #[test]
    fn normal_colon_enters_command() {
        let action = Mode::Normal.compute_mode_action(Key::Shift(':'));
        assert_eq!(action.mode, Mode::Command);
    }

    #[test]
    fn normal_dd_pending() {
        let action = Mode::Normal.compute_mode_action(Key::Char('d'));
        assert_eq!(action.mode, Mode::NormalPending(PendingKey::D));
    }

    #[test]
    fn normal_dd_executes_delete_line() {
        let action1 = Mode::Normal.compute_mode_action(Key::Char('d'));
        assert_eq!(action1.mode, Mode::NormalPending(PendingKey::D));
        let action2 = action1.mode.compute_mode_action(Key::Char('d'));
        assert_eq!(action2.mode, Mode::Normal);
        assert!(action2.command_list.is_some());
    }

    #[test]
    fn normal_yy_pending() {
        let action = Mode::Normal.compute_mode_action(Key::Char('y'));
        assert_eq!(action.mode, Mode::NormalPending(PendingKey::Y));
    }

    #[test]
    fn normal_yy_executes() {
        let action1 = Mode::Normal.compute_mode_action(Key::Char('y'));
        let action2 = action1.mode.compute_mode_action(Key::Char('y'));
        assert_eq!(action2.mode, Mode::Normal);
        assert!(action2.command_list.is_some());
    }

    #[test]
    fn normal_x_deletes() {
        let action = Mode::Normal.compute_mode_action(Key::Char('x'));
        assert_eq!(action.mode, Mode::Normal);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn normal_u_undoes() {
        let action = Mode::Normal.compute_mode_action(Key::Char('u'));
        assert_eq!(action.mode, Mode::Normal);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn normal_ctrl_s_system_save() {
        let action = Mode::Normal.compute_mode_action(Key::Ctrl('s'));
        assert_eq!(action.mode, Mode::System(System::Save));
    }

    #[test]
    fn normal_ctrl_q_system_quit() {
        let action = Mode::Normal.compute_mode_action(Key::Ctrl('q'));
        assert_eq!(action.mode, Mode::System(System::Quit));
    }

    #[test]
    fn normal_ctrl_f_system_search() {
        let action = Mode::Normal.compute_mode_action(Key::Ctrl('f'));
        assert_eq!(action.mode, Mode::System(System::Search));
    }

    #[test]
    fn normal_slash_system_search() {
        let action = Mode::Normal.compute_mode_action(Key::Char('/'));
        assert_eq!(action.mode, Mode::System(System::Search));
    }

    // --- Insert 模式 ---

    #[test]
    fn insert_escape_to_normal() {
        let action = Mode::Insert.compute_mode_action(Key::Escape);
        assert_eq!(action.mode, Mode::Normal);
    }

    #[test]
    fn insert_ctrl_w_delete_word() {
        let action = Mode::Insert.compute_mode_action(Key::Ctrl('w'));
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn insert_ctrl_u_delete_to_start() {
        let action = Mode::Insert.compute_mode_action(Key::Ctrl('u'));
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn insert_char_a() {
        let action = Mode::Insert.compute_mode_action(Key::Char('a'));
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn insert_enter() {
        let action = Mode::Insert.compute_mode_action(Key::Enter);
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn insert_backspace() {
        let action = Mode::Insert.compute_mode_action(Key::Backspace);
        assert_eq!(action.mode, Mode::Insert);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn insert_ctrl_s_system_save() {
        let action = Mode::Insert.compute_mode_action(Key::Ctrl('s'));
        assert_eq!(action.mode, Mode::System(System::Save));
    }

    // --- Visual 模式 ---

    #[test]
    fn visual_escape_to_normal() {
        let action = Mode::Visual.compute_mode_action(Key::Escape);
        assert_eq!(action.mode, Mode::Normal);
    }

    #[test]
    fn visual_d_deletes_selection() {
        let action = Mode::Visual.compute_mode_action(Key::Char('d'));
        assert_eq!(action.mode, Mode::Normal);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn visual_y_yanks() {
        let action = Mode::Visual.compute_mode_action(Key::Char('y'));
        assert_eq!(action.mode, Mode::Normal);
        assert!(action.command_list.is_some());
    }

    #[test]
    fn visual_ctrl_s_system_save() {
        let action = Mode::Visual.compute_mode_action(Key::Ctrl('s'));
        assert_eq!(action.mode, Mode::System(System::Save));
    }

    // --- CommandValue 解析 ---

    #[test]
    fn command_quit() {
        assert_eq!(CommandValue::try_from("q"), Ok(CommandValue::Quit));
    }

    #[test]
    fn command_force_quit() {
        assert_eq!(CommandValue::try_from("q!"), Ok(CommandValue::ForceQuit));
    }

    #[test]
    fn command_save() {
        assert_eq!(CommandValue::try_from("w"), Ok(CommandValue::Save));
    }

    #[test]
    fn command_save_as() {
        assert_eq!(
            CommandValue::try_from("w /tmp/test.txt"),
            Ok(CommandValue::SaveAs("/tmp/test.txt".to_string()))
        );
    }

    #[test]
    fn command_wq() {
        assert_eq!(CommandValue::try_from("wq"), Ok(CommandValue::SaveAndQuit));
    }

    #[test]
    fn command_save_as_and_quit() {
        assert_eq!(
            CommandValue::try_from("wq /tmp/test.txt"),
            Ok(CommandValue::SaveAsAndQuit("/tmp/test.txt".to_string()))
        );
    }

    #[test]
    fn command_open() {
        assert_eq!(
            CommandValue::try_from("e /tmp/test.txt"),
            Ok(CommandValue::Open("/tmp/test.txt".to_string()))
        );
    }

    #[test]
    fn command_force_open() {
        assert_eq!(
            CommandValue::try_from("e! /tmp/test.txt"),
            Ok(CommandValue::ForceOpen("/tmp/test.txt".to_string()))
        );
    }

    #[test]
    fn command_no_highlight() {
        assert_eq!(
            CommandValue::try_from("noh"),
            Ok(CommandValue::NoHighlight)
        );
    }

    #[test]
    fn command_set_line_numbers() {
        assert_eq!(
            CommandValue::try_from("set nu"),
            Ok(CommandValue::SetLineNumbers(true))
        );
    }

    #[test]
    fn command_set_no_line_numbers() {
        assert_eq!(
            CommandValue::try_from("set nonu"),
            Ok(CommandValue::SetLineNumbers(false))
        );
    }

    #[test]
    fn command_invalid() {
        assert!(CommandValue::try_from("invalid_command").is_err());
    }

    // --- FromKey trait ---

    #[test]
    fn move_from_key_hjkl() {
        assert_eq!(Move::from_key(Key::Char('h')), Ok(Move::Left));
        assert_eq!(Move::from_key(Key::Char('j')), Ok(Move::Down));
        assert_eq!(Move::from_key(Key::Char('k')), Ok(Move::Up));
        assert_eq!(Move::from_key(Key::Char('l')), Ok(Move::Right));
    }

    #[test]
    fn move_from_key_arrows() {
        assert_eq!(Move::from_key(Key::Left), Ok(Move::Left));
        assert_eq!(Move::from_key(Key::Down), Ok(Move::Down));
        assert_eq!(Move::from_key(Key::Up), Ok(Move::Up));
        assert_eq!(Move::from_key(Key::Right), Ok(Move::Right));
    }

    #[test]
    fn move_from_key_word_motions() {
        assert_eq!(Move::from_key(Key::Char('w')), Ok(Move::NextWordSplitBySymbol));
        assert_eq!(Move::from_key(Key::Char('e')), Ok(Move::EndOfWordSplitBySymbol));
        assert_eq!(Move::from_key(Key::Char('b')), Ok(Move::PrevWordSplitBySymbol));
    }

    #[test]
    fn move_from_key_line_motions() {
        assert_eq!(Move::from_key(Key::Char('0')), Ok(Move::StartOfLine));
        assert_eq!(Move::from_key(Key::Shift('$')), Ok(Move::EndOfLine));
        assert_eq!(Move::from_key(Key::Shift('^')), Ok(Move::FirstVisibleCharOfLine));
    }

    #[test]
    fn move_from_key_page() {
        assert_eq!(Move::from_key(Key::PageUp), Ok(Move::PageUp));
        assert_eq!(Move::from_key(Key::PageDown), Ok(Move::PageDown));
    }

    #[test]
    fn move_from_key_invalid() {
        assert!(Move::from_key(Key::Char('z')).is_err());
    }

    #[test]
    fn edit_from_key_insert() {
        assert_eq!(Edit::from_key(Key::Char('a')), Ok(Edit::Insert('a')));
        assert_eq!(Edit::from_key(Key::Shift('A')), Ok(Edit::Insert('A')));
    }

    #[test]
    fn edit_from_key_special() {
        assert_eq!(Edit::from_key(Key::Tab), Ok(Edit::Insert('\t')));
        assert_eq!(Edit::from_key(Key::Enter), Ok(Edit::InsertNewline));
        assert_eq!(Edit::from_key(Key::Backspace), Ok(Edit::DeleteBackward));
        assert_eq!(Edit::from_key(Key::Delete), Ok(Edit::Delete));
    }

    #[test]
    fn edit_from_key_ctrl() {
        assert_eq!(Edit::from_key(Key::CtrlBackspace), Ok(Edit::DeleteWordBackward));
        assert_eq!(Edit::from_key(Key::CtrlDelete), Ok(Edit::DeleteWord));
    }

    #[test]
    fn system_from_key() {
        assert_eq!(System::from_key(Key::Ctrl('q')), Ok(System::Quit));
        assert_eq!(System::from_key(Key::Ctrl('s')), Ok(System::Save));
        assert_eq!(System::from_key(Key::Ctrl('f')), Ok(System::Search));
        assert_eq!(System::from_key(Key::Escape), Ok(System::Dismiss));
    }

    #[test]
    fn system_from_key_invalid() {
        assert!(System::from_key(Key::Char('a')).is_err());
    }

    // --- CursorStyle ---

    #[test]
    fn cursor_style_normal_is_block() {
        assert_eq!(Mode::Normal.cursor_style(), CursorStyle::BlinkingBlock);
    }

    #[test]
    fn cursor_style_insert_is_bar() {
        assert_eq!(Mode::Insert.cursor_style(), CursorStyle::BlinkingBar);
    }

    #[test]
    fn cursor_style_visual_is_block() {
        assert_eq!(Mode::Visual.cursor_style(), CursorStyle::BlinkingBlock);
    }
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
                    // System 命令（Ctrl+S/Q/F 等）在任何模式下都生效
                    if let Ok(system) = System::from_key(key) {
                        ModeAction {
                            mode: Mode::System(system),
                            command_list: Some(vec![CommandType::System(system)]),
                        }
                    } else {
                        let command_list = Mode::try_command_list(key);
                        ModeAction {
                            mode: self,
                            command_list,
                        }
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
                    command_list: None,
                },
                Key::Shift('V') => ModeAction {
                    mode: Mode::VisualLine,
                    command_list: None,
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
                    // System 命令（Ctrl+S/Q/F 等）在任何模式下都生效
                    if let Ok(system) = System::from_key(key) {
                        ModeAction {
                            mode: Mode::System(system),
                            command_list: Some(vec![CommandType::System(system)]),
                        }
                    } else if let Ok(move_cmd) = CommandMove::from_key(key) {
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
