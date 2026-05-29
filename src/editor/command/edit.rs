use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Copy, Clone)]
pub enum Edit {
    Insert(char), InsertNewline, Delete, DeleteBackward, DeleteToEnd, DeleteWholeLine,
    DeleteWord, DeleteWordBackward, DeleteToNextWordStart, DeleteToLineStart, DeleteLine,
    DeleteRangeToMove(Move), YankRangeToMove(Move), ChangeRangeToMove(Move),
    Replace(char), YankLine, Paste, PasteAbove, Undo, Redo,
    JoinLines, ToggleCase, RepeatLastEdit, Indent, Deindent,
    DeleteSelection, YankSelection, ToggleCaseSelection, ReplaceWithYank,
    YankInnerWord,
}

use crate::editor::command::move_command::Move;

impl TryFrom<KeyEvent> for Edit {

    type Error = String;

    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        let KeyEvent { code, modifiers, ..} = event;
        match (code, modifiers) {
            (KeyCode::Char(character), KeyModifiers::NONE | KeyModifiers::SHIFT) => Ok(Self::Insert(character)),
            (KeyCode::Tab, KeyModifiers::NONE) => Ok(Self::Insert('\t')),
            (KeyCode::Enter, KeyModifiers::NONE) => Ok(Self::InsertNewline),
            (KeyCode::Backspace, KeyModifiers::NONE) => Ok(Self::DeleteBackward),
            (KeyCode::Backspace, KeyModifiers::CONTROL) => Ok(Self::DeleteWordBackward),
            (KeyCode::Delete, KeyModifiers::NONE) => Ok(Self::Delete),
            (KeyCode::Delete, KeyModifiers::CONTROL) => Ok(Self::DeleteWord),
            _ => Err(format!("Unsupported key code {code:?} or modifier {modifiers:?}"))
        }
    }
}