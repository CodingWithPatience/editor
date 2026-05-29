use crate::editor::terminal::attribute::Attribute;
use crossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};
use crossterm::style::{
    Attribute as CtAttribute, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
};
use crossterm::{queue, Command};
use editor_core::annotated_string::AnnotatedString;
use editor_core::prelude::{Position, RowIdx, Size, GUTTER_WIDTH};
use editor_core::renderer::{CursorStyle, Renderer};
use std::io::{stdout, Error, Write};

/// crossterm 渲染后端，实现 Renderer trait。
pub struct CrosstermRenderer {
    show_numbers: bool,
    cursor_line: usize,
}

impl Default for CrosstermRenderer {
    fn default() -> Self {
        Self {
            show_numbers: true,
            cursor_line: usize::MAX,
        }
    }
}

impl CrosstermRenderer {
    /// 渲染空白行号区域（用于空行和消息栏，不显示行号）。
    fn render_empty_gutter(&self) -> Result<(), Error> {
        self.set_attribute(&Attribute::from(
            editor_core::annotation_type::AnnotationType::LineNumberSection,
        ))?;
        self.queue_command(Print(format!("{:width$}", " ", width = GUTTER_WIDTH)))?;
        Ok(())
    }

    /// 渲染带行号的区域（用于有内容的行）。
    fn render_line_number(&self, line_idx: RowIdx) -> Result<(), Error> {
        if self.show_numbers {
            let annotation = if line_idx == self.cursor_line {
                editor_core::annotation_type::AnnotationType::LineNumber
            } else {
                editor_core::annotation_type::AnnotationType::LineNumberSection
            };
            self.set_attribute(&Attribute::from(annotation))?;
            self.queue_command(Print(format!(
                "{:width$} ",
                line_idx + 1,
                width = GUTTER_WIDTH - 1
            )))?;
        } else {
            self.render_empty_gutter()?;
        }
        Ok(())
    }

    fn set_attribute(&self, attribute: &Attribute) -> Result<(), Error> {
        if let Some(foreground_color) = attribute.foreground {
            self.queue_command(SetForegroundColor(foreground_color))?;
        }
        if let Some(background_color) = attribute.background {
            self.queue_command(SetBackgroundColor(background_color))?;
        }
        Ok(())
    }

    fn reset_color(&self) -> Result<(), Error> {
        self.queue_command(ResetColor)?;
        Ok(())
    }

    fn queue_command<T: Command>(&self, command: T) -> Result<(), Error> {
        queue!(stdout(), command)?;
        Ok(())
    }
}

impl Renderer for CrosstermRenderer {
    fn initialize(&mut self) -> Result<(), String> {
        enable_raw_mode().map_err(|e| e.to_string())?;
        Self::enter_alternate_screen()?;
        Self::disable_line_wrap()?;
        self.clear_screen()?;
        self.move_caret_to(Position { col: 0, row: 0 })?;
        self.flush()?;
        Ok(())
    }

    fn terminate(&mut self) -> Result<(), String> {
        Self::leave_alternate_screen()?;
        Self::enable_line_wrap()?;
        self.show_caret()?;
        self.flush()?;
        disable_raw_mode().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn size(&self) -> Result<Size, String> {
        let (width_u16, height_u16) = size().map_err(|e| e.to_string())?;
        #[allow(clippy::as_conversions)]
        let width = width_u16.saturating_sub(GUTTER_WIDTH as u16) as usize;
        #[allow(clippy::as_conversions)]
        let height = height_u16 as usize;
        Ok(Size { width, height })
    }

    fn clear_screen(&mut self) -> Result<(), String> {
        self.queue_command(Clear(ClearType::All))
            .map_err(|e| e.to_string())
    }

    fn render_text_row(&mut self, row: RowIdx, text: &str) -> Result<(), String> {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        self.queue_command(MoveTo(0, row as u16))
            .map_err(|e| e.to_string())?;
        self.queue_command(Clear(ClearType::CurrentLine))
            .and_then(|()| self.render_empty_gutter())
            .and_then(|()| self.reset_color())
            .and_then(|()| self.queue_command(Print(text)))
            .map_err(|e| e.to_string())
    }

    fn render_inverted_row(&mut self, row: RowIdx, text: &str) -> Result<(), String> {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        {
            self.queue_command(MoveTo(0, row as u16))
                .map_err(|e| e.to_string())?;
        }
        self.queue_command(Clear(ClearType::CurrentLine))
            .and_then(|()| self.render_empty_gutter())
            .and_then(|()| self.queue_command(SetAttribute(CtAttribute::Reverse)))
            .and_then(|()| self.queue_command(Print(text)))
            .and_then(|()| self.queue_command(SetAttribute(CtAttribute::Reset)))
            .map_err(|e| e.to_string())
    }

    fn render_annotated_row(
        &mut self,
        row: RowIdx,
        line_idx: usize,
        annotated_string: &AnnotatedString,
    ) -> Result<(), String> {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        self.queue_command(MoveTo(0, row as u16))
            .map_err(|e| e.to_string())?;
        self.queue_command(Clear(ClearType::CurrentLine))
            .and_then(|()| self.render_line_number(line_idx))
            .and_then(|()| self.reset_color())
            .map_err(|e| e.to_string())?;

        for part in annotated_string {
            if let Some(annotation_type) = part.annotated_type {
                let attribute: Attribute = annotation_type.into();
                self.set_attribute(&attribute).map_err(|e| e.to_string())?;
            }
            self.queue_command(Print(part.string))
                .map_err(|e| e.to_string())?;
            self.reset_color().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn set_show_line_numbers(&mut self, show: bool) {
        self.show_numbers = show;
    }

    fn set_cursor_line(&mut self, line: usize) {
        self.cursor_line = line;
    }

    fn move_caret_to(&mut self, position: Position) -> Result<(), String> {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        self.queue_command(MoveTo(
            (position.col.saturating_add(GUTTER_WIDTH)) as u16,
            position.row as u16,
        ))
        .map_err(|e| e.to_string())
    }

    fn set_cursor_style(&mut self, style: CursorStyle) -> Result<(), String> {
        let ct_style = match style {
            CursorStyle::Block => SetCursorStyle::DefaultUserShape,
            CursorStyle::Bar => SetCursorStyle::BlinkingBar,
            CursorStyle::UnderScore => SetCursorStyle::BlinkingUnderScore,
            CursorStyle::BlinkingBlock => SetCursorStyle::BlinkingBlock,
            CursorStyle::BlinkingBar => SetCursorStyle::BlinkingBar,
            CursorStyle::BlinkingUnderScore => SetCursorStyle::BlinkingUnderScore,
        };
        self.queue_command(ct_style).map_err(|e| e.to_string())
    }

    fn hide_caret(&mut self) -> Result<(), String> {
        self.queue_command(Hide).map_err(|e| e.to_string())
    }

    fn show_caret(&mut self) -> Result<(), String> {
        self.queue_command(Show).map_err(|e| e.to_string())
    }

    fn set_title(&mut self, title: &str) -> Result<(), String> {
        self.queue_command(SetTitle(title))
            .map_err(|e| e.to_string())
    }

    fn flush(&mut self) -> Result<(), String> {
        stdout().flush().map_err(|e| e.to_string())
    }
}

impl CrosstermRenderer {
    fn enter_alternate_screen() -> Result<(), String> {
        queue!(stdout(), EnterAlternateScreen).map_err(|e| e.to_string())
    }

    fn leave_alternate_screen() -> Result<(), String> {
        queue!(stdout(), LeaveAlternateScreen).map_err(|e| e.to_string())
    }

    fn enable_line_wrap() -> Result<(), String> {
        queue!(stdout(), EnableLineWrap).map_err(|e| e.to_string())
    }

    fn disable_line_wrap() -> Result<(), String> {
        queue!(stdout(), DisableLineWrap).map_err(|e| e.to_string())
    }
}
