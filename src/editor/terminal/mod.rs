mod attribute;

use crossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};
use crossterm::style::{Attribute, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size, Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen, SetTitle};
use crossterm::{queue, Command};
use std::io::{stdout, Error, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use crate::editor::annotated_string::AnnotatedString;
use crate::editor::annotation_type::AnnotationType;
use crate::prelude::{Position, RowIdx, Size};

pub const OFFSET_COL: usize = 6;

static SHOW_NUMBERS: AtomicBool = AtomicBool::new(true);
static CURSOR_LINE: AtomicUsize = AtomicUsize::new(usize::MAX);

pub struct Terminal;

impl Terminal {

    pub fn initialize() -> Result<(), Error> {
        enable_raw_mode()?;
        Self::enter_alternate_screen()?;
        Self::disable_line_wrap()?;
        Self::clear_screen()?;
        Self::move_caret_to(Position {col: 0, row: 0})?;
        Self::execute()?;
        Ok(())
    }

    pub fn clear_screen() -> Result<(), Error> {
        Self::queue_command(Clear(ClearType::All))?;
        Ok(())
    }

    pub fn clear_line() -> Result<(), Error> {
        Self::queue_command(Clear(ClearType::CurrentLine))?;
        Ok(())
    }

    pub fn hide_caret() -> Result<(), Error> {
        Self::queue_command(Hide)?;
        Ok(())
    }

    pub fn show_caret() -> Result<(), Error> {
        Self::queue_command(Show)?;
        Ok(())
    }

    pub fn terminate() -> Result<(), Error> {
        Self::leave_alternate_screen()?;
        Self::enable_line_wrap()?;
        Self::show_caret()?;
        Self::execute()?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn move_caret_to(position: Position) -> Result<(), Error> {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        Self::queue_command(MoveTo((position.col + OFFSET_COL) as u16, position.row as u16))?;
        Ok(())
    }

    fn move_caret_to_no_offset(position: Position) -> Result<(), Error> {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        Self::queue_command(MoveTo(position.col as u16, position.row as u16))?;
        Ok(())
    }

    pub fn size() -> Result<Size, Error> {
        let (width_u16, height_u16) = size()?;
        #[allow(clippy::as_conversions)]
        let width = width_u16.saturating_sub(OFFSET_COL as u16) as usize;
        #[allow(clippy::as_conversions)]
        let height = height_u16 as usize;
        Ok(Size {width, height})
    }

    pub fn enable_line_wrap() -> Result<(), Error> {
        Self::queue_command(EnableLineWrap)?;
        Ok(())
    }

    pub fn disable_line_wrap() -> Result<(), Error> {
        Self::queue_command(DisableLineWrap)?;
        Ok(())
    }

    pub fn set_title(title: &str) -> Result<(), Error> {
        Self::queue_command(SetTitle(title))?;
        Ok(())
    }

    pub fn print(string: &str) -> Result<(), Error> {
        Self::queue_command(Print(string))?;
        Ok(())
    }

    pub fn execute() -> Result<(), Error> {
        stdout().flush()?;
        Ok(())
    }

    pub fn enter_alternate_screen() -> Result<(), Error> {
        Self::queue_command(EnterAlternateScreen)?;
        Ok(())
    }

    pub fn leave_alternate_screen() -> Result<(), Error> {
        Self::queue_command(LeaveAlternateScreen)?;
        Ok(())
    }

    pub fn set_show_line_numbers(show: bool) {
        SHOW_NUMBERS.store(show, Ordering::Relaxed);
    }

    pub fn set_cursor_line(line: usize) {
        CURSOR_LINE.store(line, Ordering::Relaxed);
    }

    fn render_line_number(row: RowIdx) -> Result<(), Error> {
        if SHOW_NUMBERS.load(Ordering::Relaxed) && row == 0 {
            Self::set_attribute(&AnnotationType::LineNumber.into())?;
            Self::print(&format!("{:width$} ", row + 1, width = OFFSET_COL - 1))?;
        } else {
            Self::set_attribute(&AnnotationType::LineNumberSection.into())?;
            Self::print(&format!("{:width$}", " ", width = OFFSET_COL))?;
        }
        Ok(())
    }

    pub fn print_row(row: RowIdx, line_text: &str) -> Result<(), Error> {
        Self::move_caret_to_no_offset(Position {col: 0, row})?;
        Self::clear_line()?;
        Self::render_line_number(row)?;
        Self::reset_color()?;
        Self::print(line_text)?;
        Ok(())
    }

    pub fn print_inverted_row(row: RowIdx, line_text: &str) -> Result<(), Error> {
        let width = Self::size()?.width;
        Self::print_row(row, &format!("{}{:width$.width$}{}", Attribute::Reverse, line_text, Attribute::Reset))
    }

    pub fn print_annotated_row(row: RowIdx, line_idx: usize, annotated_string: &AnnotatedString) -> Result<(), Error> {
        Self::move_caret_to_no_offset(Position {col: 0, row})?;
        Self::clear_line()?;
        let is_cursor = line_idx == CURSOR_LINE.load(Ordering::Relaxed);
        if SHOW_NUMBERS.load(Ordering::Relaxed) {
            if is_cursor {
                Self::set_attribute(&AnnotationType::LineNumber.into())?;
            } else {
                Self::set_attribute(&AnnotationType::LineNumberSection.into())?;
            }
            Self::print(&format!("{:width$} ", line_idx + 1, width = OFFSET_COL - 1))?;
        } else {
            Self::set_attribute(&AnnotationType::LineNumberSection.into())?;
            Self::print(&format!("{:width$}", " ", width = OFFSET_COL))?;
        }
        Self::reset_color()?;
        annotated_string.into_iter()
            .try_for_each(|part| -> Result<(), Error> {
                if let Some(annotation_type) = part.annotated_type {
                    let attribute : attribute::Attribute = annotation_type.into();
                    Self::set_attribute(&attribute)?;
                };
                Self::print(part.string)?;
                Self::reset_color()?;
                Ok(())
            })?;
        Ok(())
    }
    
    pub fn set_cursor_style(cursor_style: SetCursorStyle) -> Result<(), Error> {
        Self::queue_command(cursor_style)
    }
    
    fn set_attribute(attribute: &attribute::Attribute) -> Result<(), Error> {
        if let Some(foreground_color) = attribute.foreground {
            Self::queue_command(SetForegroundColor(foreground_color))?;
        }
        if let Some(background_color) = attribute.background {
            Self::queue_command(SetBackgroundColor(background_color))?;
        }
        Ok(())
    }
    
    fn reset_color() -> Result<(), Error> {
        Self::queue_command(ResetColor)?;
        Ok(())
    }

    fn queue_command<T: Command>(command: T) -> Result<(), Error> {
        queue!(stdout(), command)?;
        Ok(())
    }

}