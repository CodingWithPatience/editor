use crate::editor::command::{Command, Edit, Move, System};
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use once_cell::sync::Lazy;
use regex::Regex;

type CommandType = Command;
type CommandEdit = Edit;
type CommandMove = Move;
type CommandSystem = System;

static PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(w|wq|e|e!)\s+([\\0-9a-zA-Z_.:/\-\p{Han}]+)$").unwrap());

pub struct ModeAction {
    pub mode: Mode,
    pub command_list: Option<Vec<CommandType>>,
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
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

#[derive(Copy, Clone, Eq, PartialEq)]
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

impl Mode {
    
    pub fn cursor_style(&self) -> SetCursorStyle {
        match *self {
            Mode::Normal => SetCursorStyle::BlinkingBlock,
            Mode::NormalPending(_) => SetCursorStyle::BlinkingUnderScore,
            Mode::Visual | Mode::VisualLine => SetCursorStyle::BlinkingBlock,
            _ => SetCursorStyle::BlinkingBar,
        }
    }
    
    pub fn compute_mode_action(self, event: KeyEvent) -> ModeAction {
        let KeyEvent { code, modifiers, ..} = event;
        match self {
            Mode::Insert => {
                match (code, modifiers) {
                    (KeyCode::Esc, KeyModifiers::NONE)
                    | (KeyCode::Char('['), KeyModifiers::CONTROL) => ModeAction {
                        mode: Mode::Normal,
                        command_list: None,
                    },
                    (KeyCode::Char('w'), KeyModifiers::CONTROL) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteWordBackward)]),
                    },
                    (KeyCode::Char('u'), KeyModifiers::CONTROL) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToLineStart)]),
                    },
                    (_, KeyModifiers::NONE) => {
                        ModeAction {
                            mode: self,
                            command_list: Mode::try_command_list(event)
                        }
                    },
                    (_, KeyModifiers::SHIFT) => {
                        let command_list = CommandEdit::try_from(event).map_or(None, |command| Some(vec![CommandType::Edit(command)]));
                        ModeAction {
                            mode: self,
                            command_list,
                        }
                    },
                    (_, KeyModifiers::CONTROL) => {
                        let command_list = CommandEdit::try_from(event).map_or(None, |command| Some(vec![CommandType::Edit(command)]));
                        ModeAction {
                            mode: self,
                            command_list,
                        }
                    },
                    _ => ModeAction {
                        mode: self,
                        command_list: None,
                    }
                }
            },
            Mode::Normal => {
                match (code, modifiers) {
                    (KeyCode::Char('i'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Insert,
                        command_list: None
                    },
                    (KeyCode::Char('I'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Move(CommandMove::StartOfLine)]),
                    },
                    (KeyCode::Char('a'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Move(CommandMove::Right)]),
                    },
                    (KeyCode::Char('A'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Move(CommandMove::EndOfLine)]),
                    },
                    (KeyCode::Char('s'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Delete)]),
                    },
                    (KeyCode::Char('S'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteWholeLine)]),
                    },
                    (KeyCode::Char('C'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToEnd)]),
                    },
                    (KeyCode::Char('D'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToEnd)]),
                    },
                    (KeyCode::Char('x'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Delete)])
                    },
                    (KeyCode::Char('X'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteBackward)])
                    },
                    (KeyCode::Char('o'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Move(CommandMove::EndOfLine), CommandType::Edit(CommandEdit::InsertNewline)]),
                    },
                    (KeyCode::Char('O'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Move(CommandMove::StartOfLine), CommandType::Edit(CommandEdit::InsertNewline), CommandType::Move(CommandMove::Up)]),
                    },
                    (KeyCode::Char('v'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Visual,
                        command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)])
                    },
                    (KeyCode::Char('V'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::VisualLine,
                        command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)])
                    },
                    (KeyCode::Char(':'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Command,
                        command_list: Some(vec![CommandType::System(CommandSystem::CommandMode)])
                    },
                    (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: Mode::System(CommandSystem::Quit),
                            command_list: Some(vec![CommandType::System(CommandSystem::Quit)])
                        }
                    },
                    (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: Mode::System(CommandSystem::Save),
                            command_list: Some(vec![CommandType::System(CommandSystem::Save)])
                        }
                    },
                    (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: Mode::System(CommandSystem::Search),
                            command_list: Some(vec![CommandType::System(CommandSystem::Search)])
                        }
                    },
                    (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: self,
                            command_list: Some(vec![CommandType::Edit(CommandEdit::Redo)])
                        }
                    },
                    (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: self,
                            command_list: Some(vec![CommandType::Move(CommandMove::HalfPageDown)])
                        }
                    },
                    (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: self,
                            command_list: Some(vec![CommandType::Move(CommandMove::HalfPageUp)])
                        }
                    },
                    (KeyCode::Char('d'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::D),
                        command_list: None,
                    },
                    (KeyCode::Char('y'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::Y),
                        command_list: None,
                    },
                    (KeyCode::Char('g'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::G),
                        command_list: None,
                    },
                    (KeyCode::Char('r'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::R),
                        command_list: None,
                    },
                    (KeyCode::Char('c'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::C),
                        command_list: None,
                    },
                    (KeyCode::Char('t'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::T),
                        command_list: None,
                    },
                    (KeyCode::Char('f'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::F),
                        command_list: None,
                    },
                    (KeyCode::Char('T'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::CapitalT),
                        command_list: None,
                    },
                    (KeyCode::Char('F'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::CapitalF),
                        command_list: None,
                    },
                    (KeyCode::Char(';'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindSameDir)]),
                    },
                    (KeyCode::Char(','), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindOppositeDir)]),
                    },
                    (KeyCode::Char('J'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::JoinLines)]),
                    },
                    (KeyCode::Char('~'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::ToggleCase)]),
                    },
                    (KeyCode::Char('.'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::RepeatLastEdit)]),
                    },
                    (KeyCode::Char('<'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::Lt),
                        command_list: None,
                    },
                    (KeyCode::Char('>'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::Gt),
                        command_list: None,
                    },
                    (KeyCode::Char('/'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::System(CommandSystem::Search),
                        command_list: Some(vec![CommandType::System(CommandSystem::Search)]),
                    },
                    (KeyCode::Char('?'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::System(CommandSystem::SearchBackward),
                        command_list: Some(vec![CommandType::System(CommandSystem::SearchBackward)]),
                    },
                    (KeyCode::Char('*'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::System(CommandSystem::SearchWordForward)]),
                    },
                    (KeyCode::Char('#'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::System(CommandSystem::SearchWordBackward)]),
                    },
                    (KeyCode::Char('%'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::MatchBracket)]),
                    },
                    (KeyCode::Char('p'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Paste)]),
                    },
                    (KeyCode::Char('Y'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::YankLine)]),
                    },
                    (KeyCode::Char('P'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::PasteAbove)]),
                    },
                    (KeyCode::Char('u'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Undo)]),
                    },
                    (KeyCode::Char('n'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::SearchNext)]),
                    },
                    (KeyCode::Char('N'), KeyModifiers::SHIFT) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::SearchPrev)]),
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                        let command_list = CommandMove::try_from(event).map_or(None, |command| Some(vec![CommandType::Move(command)]));
                        ModeAction {
                            mode: self,
                            command_list,
                        }
                    },
                    _ => ModeAction {
                        mode: self,
                        command_list: None
                    }
                }
            },
            Mode::NormalPending(pending_key) => {
                match (code, modifiers, pending_key) {
                    (KeyCode::Esc, KeyModifiers::NONE, _)
                    | (KeyCode::Char('['), KeyModifiers::CONTROL, _) => ModeAction {
                        mode: Mode::Normal,
                        command_list: None,
                    },
                    // d: dd (line) + dw (word) + d<any-move>
                    (KeyCode::Char('d'), KeyModifiers::NONE, PendingKey::D) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteLine)]),
                    },
                    (KeyCode::Char('w'), KeyModifiers::NONE, PendingKey::D) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToNextWordStart)]),
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::D) => {
                        if let Ok(move_cmd) = CommandMove::try_from(event) {
                            ModeAction {
                                mode: Mode::Normal,
                                command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteRangeToMove(move_cmd))]),
                            }
                        } else {
                            ModeAction { mode: Mode::Normal, command_list: None }
                        }
                    },
                    // y: yy (line) + y<any-move> + yiw (inner word)
                    (KeyCode::Char('y'), KeyModifiers::NONE, PendingKey::Y) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::YankLine)]),
                    },
                    (KeyCode::Char('i'), KeyModifiers::NONE, PendingKey::Y) => ModeAction {
                        mode: Mode::NormalPending(PendingKey::YiTextObject),
                        command_list: None,
                    },
                    // YiTextObject: yank inner text object (iw = inner word)
                    (KeyCode::Char('w'), KeyModifiers::NONE, PendingKey::YiTextObject) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::YankInnerWord)]),
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::Y) => {
                        if let Ok(move_cmd) = CommandMove::try_from(event) {
                            ModeAction {
                                mode: Mode::Normal,
                                command_list: Some(vec![CommandType::Edit(CommandEdit::YankRangeToMove(move_cmd))]),
                            }
                        } else {
                            ModeAction { mode: Mode::Normal, command_list: None }
                        }
                    },
                    // c: cc (change line) + cw (change word) + c<any-move>
                    (KeyCode::Char('c'), KeyModifiers::NONE, PendingKey::C) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteWholeLine)]),
                    },
                    (KeyCode::Char('w'), KeyModifiers::NONE, PendingKey::C) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteToNextWordStart)]),
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::C) => {
                        if let Ok(move_cmd) = CommandMove::try_from(event) {
                            ModeAction {
                                mode: Mode::Insert,
                                command_list: Some(vec![CommandType::Edit(CommandEdit::ChangeRangeToMove(move_cmd))]),
                            }
                        } else {
                            ModeAction { mode: Mode::Normal, command_list: None }
                        }
                    },
                    // g + g
                    (KeyCode::Char('g'), KeyModifiers::NONE, PendingKey::G) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::FirstLine)]),
                    },
                    // r + char
                    (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::R) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Replace(ch))]),
                    },
                    // f/F/t/T + char
                    (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::F) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::FindChar(ch))]),
                    },
                    (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::CapitalF) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::FindCharBackward(ch))]),
                    },
                    (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::T) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::TillChar(ch))]),
                    },
                    (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT, PendingKey::CapitalT) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Move(CommandMove::TillCharBackward(ch))]),
                    },
                    // << (indent)
                    (KeyCode::Char('<'), KeyModifiers::SHIFT, PendingKey::Lt) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Deindent)]),
                    },
                    // >> (deindent)
                    (KeyCode::Char('>'), KeyModifiers::SHIFT, PendingKey::Gt) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Indent)]),
                    },
                    _ => ModeAction {
                        mode: Mode::Normal,
                        command_list: None,
                    },
                }
            }
            Mode::Visual | Mode::VisualLine => {
                match (code, modifiers) {
                    (KeyCode::Esc, KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (KeyCode::Char(':'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Command,
                        command_list: Some(vec![CommandType::System(CommandSystem::CommandMode)]),
                    },
                    (KeyCode::Char('v'), KeyModifiers::NONE) => {
                        if matches!(self, Mode::VisualLine) {
                            ModeAction { mode: Mode::Visual, command_list: None }
                        } else {
                            ModeAction { mode: Mode::Normal, command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)]) }
                        }
                    },
                    (KeyCode::Char('V'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::VisualLine, command_list: None,
                    },
                    (KeyCode::Char('d'), KeyModifiers::NONE) | (KeyCode::Char('x'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteSelection), CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (KeyCode::Char('y'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::YankSelection), CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (KeyCode::Char(';'), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindSameDir)]),
                    },
                    (KeyCode::Char(','), KeyModifiers::NONE) => ModeAction {
                        mode: self,
                        command_list: Some(vec![CommandType::Move(CommandMove::RepeatFindOppositeDir)]),
                    },
                    (KeyCode::Char('c'), KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Insert,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::DeleteSelection)]),
                    },
                    (KeyCode::Char('~'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::ToggleCaseSelection), CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (KeyCode::Char('p'), KeyModifiers::NONE) | (KeyCode::Char('P'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::ReplaceWithYank), CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (KeyCode::Char('>'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Indent), CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (KeyCode::Char('<'), KeyModifiers::SHIFT) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::Edit(CommandEdit::Deindent), CommandType::System(CommandSystem::Dismiss)]),
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                        if let Ok(move_cmd) = CommandMove::try_from(event) {
                            ModeAction { mode: self, command_list: Some(vec![CommandType::Move(move_cmd)]) }
                        } else {
                            ModeAction { mode: self, command_list: None }
                        }
                    },
                    _ => ModeAction { mode: self, command_list: None },
                }
            }
            Mode::Command => {
                match (code, modifiers) {
                    (KeyCode::Esc, KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)])
                    },
                    (KeyCode::Enter, KeyModifiers::NONE) => {
                        ModeAction {
                            mode: Mode::Normal,
                            command_list: Some(vec![CommandType::Edit(CommandEdit::InsertNewline)])
                        }
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                        ModeAction {
                            mode: self,
                            command_list: Mode::try_command_list(event)
                        }
                    },
                    _ => ModeAction {
                        mode: self,
                        command_list: None
                    }
                }
            }
            Mode::System(CommandSystem::Save | CommandSystem::Search | CommandSystem::SearchBackward) => {
                match (code, modifiers) {
                    (KeyCode::Esc, KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)])
                    },
                    (_, KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                        ModeAction {
                            mode: self,
                            command_list: Mode::try_command_list(event)
                        }
                    },
                    _ => ModeAction {
                        mode: self,
                        command_list: None
                    }
                }
            }
            Mode::System(CommandSystem::Quit) => {
                match (code, modifiers) {
                    (KeyCode::Esc, KeyModifiers::NONE) => ModeAction {
                        mode: Mode::Normal,
                        command_list: Some(vec![CommandType::System(CommandSystem::Dismiss)])
                    },
                    (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                        ModeAction {
                            mode: self,
                            command_list: Some(vec![CommandType::System(CommandSystem::Quit)])
                        }
                    },
                    _ => ModeAction {
                        mode: self,
                        command_list: None
                    }
                }
            }
            Mode::System(_) => {
                ModeAction {
                    mode: self,
                    command_list: None
                }
            }
        }
    }

    fn try_command_list(event: KeyEvent) -> Option<Vec<CommandType>> {
        let mut command_list = CommandEdit::try_from(event).map_or(None, |command| Some(vec![CommandType::Edit(command)]));
        if matches!(command_list, None) {
            command_list = CommandMove::try_from(event).map_or(None, |command| Some(vec![CommandType::Move(command)]))
        }
        command_list
    }
    
}

#[derive(Debug, Eq, PartialEq)]
pub enum CommandValue {
    Quit, ForceQuit, Save, SaveAs(String), SaveAndQuit, SaveAsAndQuit(String), Open(String), ForceOpen(String),
    NoHighlight, SetLineNumbers(bool),
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
                        _ => Err(format!("Invalid command: {value:?}"))
                    }
                } else {
                    Err(format!("Invalid command: {value:?}"))
                }
            }
        }
    }
}