use crate::editor::ui_components::ui_component::UIComponent;
use editor_core::command::edit::Edit;
use editor_core::line::Line;
use editor_core::prelude::{ColIdx, GraphemeIdx, RowIdx, Size};
use editor_core::renderer::Renderer;
use std::cmp::min;

#[derive(Default)]
pub struct CommandBar {
    prompt: Line,
    value: Line,
    needs_redraw: bool,
    size: Size,
    grapheme_idx: GraphemeIdx,
}

impl CommandBar {
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => {
                let old_len = self.value.grapheme_count();
                self.value.append_char(character);
                let new_len = self.value.grapheme_count();
                if new_len.saturating_sub(old_len) > 0 {
                    self.move_right();
                }
            }
            Edit::Delete => {
                if self.grapheme_idx < self.value.grapheme_count() {
                    self.value.delete(self.grapheme_idx);
                }
            }
            Edit::DeleteBackward => {
                if self.grapheme_idx > 0 {
                    self.move_left();
                    self.value.delete(self.grapheme_idx);
                }
            }
            Edit::InsertNewline | Edit::DeleteToEnd | Edit::DeleteWholeLine => {}
            Edit::DeleteWord | Edit::DeleteWordBackward => {}
            Edit::DeleteLine
            | Edit::DeleteToLineStart
            | Edit::Replace(_)
            | Edit::YankLine
            | Edit::Paste
            | Edit::PasteAbove
            | Edit::Undo
            | Edit::Redo
            | Edit::DeleteToNextWordStart
            | Edit::DeleteRangeToMove(_)
            | Edit::YankRangeToMove(_)
            | Edit::ChangeRangeToMove(_)
            | Edit::JoinLines
            | Edit::ToggleCase
            | Edit::RepeatLastEdit
            | Edit::Indent
            | Edit::Deindent
            | Edit::DeleteSelection
            | Edit::YankSelection
            | Edit::ToggleCaseSelection
            | Edit::ReplaceWithYank
            | Edit::YankInnerWord => {}
        }
        self.set_needs_redraw(true);
    }

    pub fn move_right(&mut self) {
        self.grapheme_idx = min(self.grapheme_idx + 1, self.value.grapheme_count());
    }

    pub fn move_left(&mut self) {
        self.grapheme_idx = self.grapheme_idx.saturating_sub(1);
    }

    pub fn caret_position_col(&self) -> ColIdx {
        let prompt_width = self.prompt.width();
        let value_width = self.value.width_until(self.grapheme_idx);
        let max_width = prompt_width.saturating_add(value_width);
        min(max_width, self.size.width)
    }

    pub fn value(&self) -> String {
        self.value.to_string()
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = Line::from(prompt);
        self.set_needs_redraw(true);
    }

    pub fn clear_value(&mut self) {
        self.value = Line::default();
        self.grapheme_idx = self.value.grapheme_count();
        self.set_needs_redraw(true);
    }
}

impl UIComponent for CommandBar {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, renderer: &mut dyn Renderer, origin_row: RowIdx) -> Result<(), String> {
        let area_for_value = self.size.width.saturating_sub(self.prompt.width());
        let value_end = self.value.width();
        let value_start = value_end.saturating_sub(area_for_value);
        let message = format!(
            "{}{}",
            &self.prompt,
            self.value.get_visible_graphemes(value_start..value_end)
        );
        let to_print = if message.len() <= self.size.width {
            message
        } else {
            String::new()
        };
        renderer.render_text_row(origin_row, &to_print)
    }
}
