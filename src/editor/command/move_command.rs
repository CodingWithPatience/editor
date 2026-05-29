use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Copy, Clone)]
pub enum Move {
    PageUp,
    PageDown,
    StartOfLine,
    EndOfLine,
    Up,
    Down,
    Left,
    Right,
    FirstVisibleCharOfLine,
    FirstLine,
    LastLine,
    NextWordSplitBySymbol,
    NextWordSplitByWhitespace,
    EndOfWordSplitBySymbol,
    EndOfWordSplitByWhitespace,
    PrevWordSplitBySymbol,
    PrevWordSplitByWhitespace,
    SearchNext,
    SearchPrev,
    HalfPageDown,
    HalfPageUp,
    FindChar(char),
    FindCharBackward(char),
    TillChar(char),
    TillCharBackward(char),
    RepeatFindSameDir,
    RepeatFindOppositeDir,
    MatchBracket,
}

impl TryFrom<KeyEvent> for Move {

    type Error = String;

    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        let KeyEvent { code, modifiers, ..} = event;
        if modifiers == KeyModifiers::NONE {
            match code {
                KeyCode::Up => Ok(Self::Up),
                KeyCode::Down => Ok(Self::Down),
                KeyCode::Left => Ok(Self::Left),
                KeyCode::Right => Ok(Self::Right),
                KeyCode::PageUp => Ok(Self::PageUp),
                KeyCode::PageDown => Ok(Self::PageDown),
                KeyCode::Home => Ok(Self::StartOfLine),
                KeyCode::End => Ok(Self::EndOfLine),
                KeyCode::Char(' ') => Ok(Self::Right),
                KeyCode::Char('h') => Ok(Self::Left),
                KeyCode::Char('l') => Ok(Self::Right),
                KeyCode::Char('j') => Ok(Self::Down),
                KeyCode::Char('k') => Ok(Self::Up),
                KeyCode::Char('0') => Ok(Self::StartOfLine),
                KeyCode::Char('w') => Ok(Self::NextWordSplitBySymbol),
                KeyCode::Char('e') => Ok(Self::EndOfWordSplitBySymbol),
                KeyCode::Char('b') => Ok(Self::PrevWordSplitBySymbol),
                _ => Err(format!("Unsupported code: {code:?}"))
            }
        } else if modifiers == KeyModifiers::SHIFT {
            match code {
                KeyCode::Char('^') => Ok(Self::FirstVisibleCharOfLine),
                KeyCode::Char('$') => Ok(Self::EndOfLine),
                KeyCode::Char('G') => Ok(Self::LastLine),
                KeyCode::Char('W') => Ok(Self::NextWordSplitByWhitespace),
                KeyCode::Char('E') => Ok(Self::EndOfWordSplitByWhitespace),
                KeyCode::Char('B') => Ok(Self::PrevWordSplitByWhitespace),
                _ => Err(format!("Unsupported code: {code:?}"))
            }
        } else {
            Err(format!("Unsupported key code {code:?} or modifier {modifiers:?}"))
        }
    }
}