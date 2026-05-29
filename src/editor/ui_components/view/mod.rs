mod buffer;
mod file_info;
mod search_info;
mod search_direction;
mod highlighter;

use crate::editor::annotation_type::AnnotationType;
use crate::editor::command::edit::Edit;
use crate::editor::command::mode::{CommandValue, Mode};
use crate::editor::command::move_command::Move;
use crate::editor::document_status::DocumentStatus;
use crate::editor::line::Line;
use crate::editor::terminal::Terminal;
use crate::editor::ui_components::ui_component::UIComponent;
use crate::editor::ui_components::view::buffer::Buffer;
use crate::editor::ui_components::view::highlighter::Highlighter;
use crate::prelude::{ColIdx, Location, Position, RowIdx, Size, WordSeparatorType};
use search_direction::SearchDirection;
use search_info::SearchInfo;
use std::cmp::min;
use std::io::Error;

const MAX_UNDO_DEPTH: usize = 1000;

/// Indent width in spaces. TODO: make this configurable via `:set shiftwidth=N` or `:set sw=N`.
const INDENT_WIDTH: usize = 2;

#[derive(Copy, Clone, Default)]
enum FindKind {
    #[default]
    FindForward,
    FindBackward,
    TillForward,
    TillBackward,
}

#[derive(Clone)]
struct BufferSnapshot {
    lines: Vec<String>,
    cursor: Location,
    scroll_offset: Position,
}

#[derive(Default)]
pub struct View {
    buffer: Buffer,
    needs_redraw: bool,
    size: Size,
    text_location: Location,
    scroll_offset: Position,
    search_info: Option<SearchInfo>,
    command_info: Option<Line>,
    mode: Mode,
    yank_register: Option<String>,
    yank_register_linewise: bool,
    undo_stack: Vec<BufferSnapshot>,
    redo_stack: Vec<BufferSnapshot>,
    pub last_search_query: Option<Line>,
    pub last_search_direction: SearchDirection,
    last_find_char: Option<char>,
    last_find_kind: FindKind,
    last_edit: Option<Edit>,
    selection_anchor: Option<Location>,
    selection_linewise: bool,
}

impl View {

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        match mode {
            Mode::Visual | Mode::VisualLine => {
                self.selection_anchor = Some(self.text_location);
                self.selection_linewise = matches!(mode, Mode::VisualLine);
            }
            _ => {} // Don't clear here — Dismiss/Esc and operation methods handle cleanup
        }
        self.snap_to_valid_grapheme();
        self.set_needs_redraw(true);
    }

    pub fn get_status(&self) -> DocumentStatus {
        let file_info = self.buffer.get_file_info();
        let sel_count = self.selection_range().map(|(s, e)| {
            if self.selection_linewise {
                e.line_idx.saturating_sub(s.line_idx).saturating_add(1)
            } else {
                let mut count = 0;
                for li in s.line_idx..=e.line_idx {
                    let start_g = if li == s.line_idx { s.grapheme_idx } else { 0 };
                    let end_g = if li == e.line_idx { e.grapheme_idx } else { self.buffer.grapheme_count(li) };
                    count += end_g.saturating_sub(start_g);
                }
                count
            }
        });
        DocumentStatus {
            total_lines: self.buffer.height(),
            current_line_idx: self.text_location.line_idx,
            is_modified: self.buffer.is_dirty(),
            filename: format!("{file_info}"),
            file_type: file_info.get_file_type(),
            selection_count: sel_count,
        }
    }

    pub fn is_file_loaded(&self) -> bool {
        self.buffer.is_file_loaded()
    }
    
    pub fn handle_edit_command(&mut self, command: Edit) {
        // Save non-repeat edits for . command
        if !matches!(command, Edit::RepeatLastEdit | Edit::Undo | Edit::Redo | Edit::DeleteSelection | Edit::YankSelection) {
            self.last_edit = Some(command);
        }
        match command {
            Edit::Insert(character) => self.insert_char(character),
            Edit::InsertNewline => { self.take_snapshot(); self.insert_newline(); }
            Edit::Delete => { self.take_snapshot(); self.delete(); }
            Edit::DeleteBackward => { self.take_snapshot(); self.delete_backward(); }
            Edit::DeleteToEnd => {
                self.take_snapshot();
                let current_grapheme_idx = self.text_location.grapheme_idx;
                let max_grapheme_idx = self.buffer.grapheme_count(self.text_location.line_idx);
                for _ in current_grapheme_idx..max_grapheme_idx {
                    self.delete();
                }
            }
            Edit::DeleteWholeLine => {
                self.take_snapshot();
                let max_grapheme_idx = self.buffer.grapheme_count(self.text_location.line_idx);
                for _ in 0..max_grapheme_idx {
                    self.delete();
                }
            }
            Edit::DeleteWord => {
                self.take_snapshot();
                self.text_location = self.buffer.delete_word_forward(self.text_location);
                self.snap_to_valid_grapheme();
                self.set_needs_redraw(true);
            }
            Edit::DeleteWordBackward => {
                self.take_snapshot();
                self.text_location = self.buffer.delete_word_backward(self.text_location);
                self.snap_to_valid_grapheme();
                self.set_needs_redraw(true);
            }
            Edit::DeleteToLineStart => self.delete_to_line_start(),
            Edit::DeleteLine => self.delete_line(),
            Edit::Replace(ch) => self.replace_char(ch),
            Edit::YankLine => self.yank_line(),
            Edit::Paste => self.paste(),
            Edit::PasteAbove => self.paste_above(),
            Edit::DeleteToNextWordStart => self.delete_to_next_word_start(),
            Edit::DeleteRangeToMove(cmd) => self.delete_range_to_motion(cmd),
            Edit::YankRangeToMove(cmd) => self.yank_range_to_motion(cmd),
            Edit::ChangeRangeToMove(cmd) => self.change_range_to_motion(cmd),
            Edit::JoinLines => self.join_lines(),
            Edit::ToggleCase => self.toggle_case(),
            Edit::RepeatLastEdit => self.repeat_last_edit(),
            Edit::Indent => self.indent_line(),
            Edit::Deindent => self.deindent_line(),
            Edit::DeleteSelection => self.delete_selection(),
            Edit::YankSelection => self.yank_selection(),
            Edit::ToggleCaseSelection => self.toggle_case_selection(),
            Edit::ReplaceWithYank => self.replace_selection_with_yank(),
            Edit::YankInnerWord => self.yank_inner_word(),
            Edit::Undo => self.undo(),
            Edit::Redo => self.redo(),
        }
    }

    pub fn handle_move_command(&mut self, command: Move) {
        let Size { height, ..} = self.size;
        match command {
            Move::Up => self.move_up(1),
            Move::Down => self.move_down(1),
            Move::Left => self.move_left(),
            Move::Right => self.move_right(),
            Move::PageUp => self.move_up(height.saturating_sub(1)),
            Move::PageDown => self.move_down(height.saturating_sub(1)),
            Move::StartOfLine => self.move_to_start_of_line(),
            Move::EndOfLine => self.move_to_end_of_line(),
            Move::FirstVisibleCharOfLine => self.move_to_first_visible_of_line(),
            Move::LastLine => self.move_to_last_line(),
            Move::NextWordSplitBySymbol => self.move_to_next_word_start(WordSeparatorType::Symbol),
            Move::NextWordSplitByWhitespace => self.move_to_next_word_start(WordSeparatorType::Whitespace),
            Move::EndOfWordSplitBySymbol => self.move_to_current_or_next_word_end(WordSeparatorType::Symbol),
            Move::EndOfWordSplitByWhitespace => self.move_to_current_or_next_word_end(WordSeparatorType::Whitespace),
            Move::PrevWordSplitBySymbol => self.move_to_current_or_prev_word_start(WordSeparatorType::Symbol),
            Move::PrevWordSplitByWhitespace => self.move_to_current_or_prev_word_start(WordSeparatorType::Whitespace),
            Move::FirstLine => self.move_to_first_line(),
            Move::SearchNext => self.repeat_search(SearchDirection::Forward),
            Move::SearchPrev => self.repeat_search(SearchDirection::Backward),
            Move::HalfPageDown => self.move_down((height / 2).max(1)),
            Move::HalfPageUp => self.move_up((height / 2).max(1)),
            Move::FindChar(ch) => self.move_find_char(ch, FindKind::FindForward),
            Move::FindCharBackward(ch) => self.move_find_char(ch, FindKind::FindBackward),
            Move::TillChar(ch) => self.move_find_char(ch, FindKind::TillForward),
            Move::TillCharBackward(ch) => self.move_find_char(ch, FindKind::TillBackward),
            Move::RepeatFindSameDir => self.repeat_find_same_dir(),
            Move::RepeatFindOppositeDir => self.repeat_find_opposite_dir(),
            Move::MatchBracket => self.move_to_matching_bracket(),
        }
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    fn move_up(&mut self, step: usize) {
        self.text_location.line_idx = self.text_location.line_idx.saturating_sub(step);
        self.snap_to_valid_grapheme();
    }

    fn move_down(&mut self, step: usize) {
        if self.buffer.height().saturating_sub(1) > self.text_location.line_idx {
            self.text_location.line_idx = self.text_location.line_idx.saturating_add(step);
            self.snap_to_valid_grapheme();
            self.snap_to_valid_line();
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn move_left(&mut self) {
        if self.text_location.grapheme_idx > 0 {
            self.text_location.grapheme_idx -= 1;
        }
    }

    fn move_right(&mut self) {
        let mut grapheme_count = self.buffer.grapheme_count(self.text_location.line_idx);
        if matches!(self.mode, Mode::Normal) {
            grapheme_count = grapheme_count.saturating_sub(1);
        }
        if self.text_location.grapheme_idx < grapheme_count {
            self.text_location.grapheme_idx += 1;
        }
    }

    fn move_to_start_of_line(&mut self) {
        self.text_location.grapheme_idx = 0;
    }

    fn move_to_end_of_line(&mut self) {
        self.text_location.grapheme_idx = self.buffer.grapheme_count(self.text_location.line_idx);
        if matches!(self.mode, Mode::Normal) {
            self.move_left();
        }
    }

    fn move_to_first_visible_of_line(&mut self) {
        self.text_location.grapheme_idx = self.buffer.first_visible_of_line_grapheme_idx(self.text_location.line_idx);
    }

    fn move_to_last_line(&mut self) {
        self.text_location.grapheme_idx = 0;
        self.text_location.line_idx = self.buffer.last_line_idx();
    }

    fn move_to_next_word_start(&mut self, separator_type: WordSeparatorType) {
        self.text_location = self.buffer.next_word_location_start(&self.text_location, separator_type);
    }

    fn move_to_current_or_next_word_end(&mut self, separator_type: WordSeparatorType) {
        self.text_location = self.buffer.current_or_next_word_location_end(&self.text_location, separator_type);
    }

    fn move_to_current_or_prev_word_start(&mut self, separator_type: WordSeparatorType) {
        self.text_location = self.buffer.current_or_prev_word_location_start(&self.text_location, separator_type);
    }

    /// Move cursor to the next/previous occurrence of `ch` on the current line.
    /// Also records the search for `;` / `,` repeat.
    fn move_find_char(&mut self, ch: char, kind: FindKind) {
        self.last_find_char = Some(ch);
        self.last_find_kind = kind;
        let line_idx = self.text_location.line_idx;
        let from = self.text_location.grapheme_idx;
        let result = match kind {
            FindKind::FindForward => self.buffer.find_char_on_line(line_idx, from, ch, true),
            FindKind::FindBackward => self.buffer.find_char_on_line(line_idx, from, ch, false),
            FindKind::TillForward => {
                self.buffer.find_char_on_line(line_idx, from, ch, true).and_then(|idx| {
                    if idx > 0 { Some(idx.saturating_sub(1)) } else { None }
                })
            }
            FindKind::TillBackward => {
                self.buffer.find_char_on_line(line_idx, from, ch, false).and_then(|idx| {
                    let count = self.buffer.grapheme_count(line_idx);
                    if idx < count.saturating_sub(1) { Some(idx.saturating_add(1)) } else { None }
                })
            }
        };
        if let Some(idx) = result {
            self.text_location.grapheme_idx = idx;
            self.snap_to_valid_grapheme();
        }
    }

    /// Repeat the last f/F/t/T search in the same direction.
    fn repeat_find_same_dir(&mut self) {
        if let Some(ch) = self.last_find_char {
            let kind = self.last_find_kind;
            self.move_find_char(ch, kind);
        }
    }

    /// Repeat the last f/F/t/T search in the opposite direction.
    fn repeat_find_opposite_dir(&mut self) {
        if let Some(ch) = self.last_find_char {
            let opposite = match self.last_find_kind {
                FindKind::FindForward => FindKind::FindBackward,
                FindKind::FindBackward => FindKind::FindForward,
                FindKind::TillForward => FindKind::TillBackward,
                FindKind::TillBackward => FindKind::TillForward,
            };
            self.move_find_char(ch, opposite);
        }
    }

    /// Join the next line onto the end of the current line.
    fn join_lines(&mut self) {
        self.take_snapshot();
        let line_idx = self.text_location.line_idx;
        if line_idx < self.buffer.last_line_idx() {
            self.move_to_end_of_line();
            self.buffer.join_next_line(line_idx);
            self.set_needs_redraw(true);
        }
    }

    /// Toggle ASCII case of the character under the cursor, then move right.
    fn toggle_case(&mut self) {
        self.take_snapshot();
        let saved = self.text_location;
        if let Some(line) = self.buffer.get_line_content(saved.line_idx) {
            let byte_idx = self.buffer.grapheme_to_byte(saved.line_idx, saved.grapheme_idx);
            if let Some(&ch) = line.as_bytes().get(byte_idx) {
                let toggled = if ch.is_ascii_uppercase() {
                    ch.to_ascii_lowercase() as char
                } else if ch.is_ascii_lowercase() {
                    ch.to_ascii_uppercase() as char
                } else {
                    return;
                };
                self.delete();
                // insert at original position — delete() may have moved cursor back (Normal mode)
                self.buffer.insert_char(toggled, saved);
                self.text_location = saved;
                self.move_right();
                self.set_needs_redraw(true);
            }
        }
    }

    /// Repeat the last non-repeat edit operation.
    fn repeat_last_edit(&mut self) {
        if let Some(cmd) = self.last_edit {
            self.handle_edit_command(cmd);
        }
    }

    /// Insert INDENT_WIDTH spaces at the beginning of the current line.
    fn indent_line(&mut self) {
        self.take_snapshot();
        let visual = self.selection_anchor.is_some();
        let (from, to) = if visual {
            self.selection_range().map_or(
                (self.text_location.line_idx, self.text_location.line_idx),
                |(s, e)| (s.line_idx, e.line_idx),
            )
        } else {
            (self.text_location.line_idx, self.text_location.line_idx)
        };
        for line_idx in from..=to {
            for _ in 0..INDENT_WIDTH {
                self.buffer.insert_char(' ', Location { line_idx, grapheme_idx: 0 });
            }
        }
        if visual { self.clear_selection(); }
        self.move_to_first_visible_of_line();
        self.set_needs_redraw(true);
    }

    /// Remove leading indent from current line or visual selection.
    fn deindent_line(&mut self) {
        self.take_snapshot();
        let visual = self.selection_anchor.is_some();
        let (from, to) = if visual {
            self.selection_range().map_or(
                (self.text_location.line_idx, self.text_location.line_idx),
                |(s, e)| (s.line_idx, e.line_idx),
            )
        } else {
            (self.text_location.line_idx, self.text_location.line_idx)
        };
        for line_idx in from..=to {
            if let Some(line) = self.buffer.get_line_content(line_idx) {
                let count = if line.starts_with('\t') {
                    1
                } else if line.starts_with(&" ".repeat(INDENT_WIDTH)) {
                    INDENT_WIDTH
                } else {
                    continue;
                };
                for _ in 0..count {
                    self.text_location = Location { line_idx, grapheme_idx: 0 };
                    self.delete();
                }
            }
        }
        if visual { self.clear_selection(); }
        self.move_to_first_visible_of_line();
        self.set_needs_redraw(true);
    }

    /// Delete the visual selection and return to Normal mode.
    fn delete_selection(&mut self) {
        self.take_snapshot();
        if let Some((start, end)) = self.selection_range() {
            if self.selection_linewise {
                let mut deleted = String::new();
                for line_idx in (start.line_idx..=end.line_idx).rev() {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        deleted.insert_str(0, &line);
                        deleted.insert(0, '\n');
                    }
                    self.buffer.delete_line(line_idx);
                }
                if !deleted.is_empty() { deleted.remove(0); } // remove leading '\n'
                self.yank_register = Some(deleted);
                self.yank_register_linewise = true;
                if self.buffer.height() == 0 {
                    let loc = Location { line_idx: 0, grapheme_idx: 0 };
                    self.buffer.insert_char(' ', loc);
                    self.buffer.delete(loc);
                }
                self.text_location = Location { line_idx: start.line_idx.min(self.buffer.last_line_idx()), grapheme_idx: 0 };
            } else {
                self.text_location = start;
                let mut deleted = String::new();
                if start.line_idx == end.line_idx {
                    // Same line: compute count upfront to avoid infinite loop from shifting buffer
                    let count = end.grapheme_idx.saturating_sub(start.grapheme_idx);
                    // Collect text before deleting
                    if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                        let sb = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                        let eb = self.buffer.grapheme_to_byte(start.line_idx, end.grapheme_idx);
                        if sb < eb && eb <= line.len() {
                            deleted.push_str(&line[sb..eb]);
                        }
                    }
                    for _ in 0..count {
                        self.delete();
                    }
                } else {
                    // Cross-line: use delete_range_inner which handles multi-line correctly
                    self.text_location = start;
                    let loc_end = end;
                    let mut line_idx = start.line_idx;
                    while line_idx <= loc_end.line_idx {
                        if let Some(line) = self.buffer.get_line_content(line_idx) {
                            let s = if line_idx == start.line_idx {
                                self.buffer.grapheme_to_byte(line_idx, start.grapheme_idx)
                            } else { 0 };
                            let e = if line_idx == loc_end.line_idx {
                                self.buffer.grapheme_to_byte(line_idx, loc_end.grapheme_idx)
                            } else { line.len() };
                            if s < e && e <= line.len() {
                                deleted.push_str(&line[s..e]);
                            }
                            if line_idx < loc_end.line_idx {
                                deleted.push('\n');
                            }
                        }
                        line_idx += 1;
                    }
                    self.delete_range_inner(start, loc_end);
                }
                if !deleted.is_empty() {
                    self.yank_register = Some(deleted);
                    self.yank_register_linewise = false;
                }
            }
        }
        self.clear_selection();
        self.snap_to_valid_grapheme();
        self.snap_to_valid_line();
        self.set_needs_redraw(true);
    }

    /// Copy the visual selection to the yank register and return to Normal mode.
    fn yank_selection(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            let mut text = String::new();
            if self.selection_linewise {
                for line_idx in start.line_idx..=end.line_idx {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        text.push_str(&line);
                        if line_idx < end.line_idx {
                            text.push('\n');
                        }
                    }
                }
            } else {
                for line_idx in start.line_idx..=end.line_idx {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        let s = if line_idx == start.line_idx {
                            self.buffer.grapheme_to_byte(line_idx, start.grapheme_idx)
                        } else { 0 };
                        let e = if line_idx == end.line_idx {
                            self.buffer.grapheme_to_byte(line_idx, end.grapheme_idx)
                        } else { line.len() };
                        if s < e && e <= line.len() {
                            text.push_str(&line[s..e]);
                        }
                    }
                    if line_idx < end.line_idx {
                        text.push('\n');
                    }
                }
            }
            if !text.is_empty() {
                self.yank_register = Some(text);
                self.yank_register_linewise = self.selection_linewise;
            }
        }
        self.clear_selection();
        self.set_needs_redraw(true);
    }

    /// Toggle ASCII case of all characters in the visual selection.
    fn toggle_case_selection(&mut self) {
        self.take_snapshot();
        if let Some((start, end)) = self.selection_range() {
            if self.selection_linewise {
                for line_idx in start.line_idx..=end.line_idx {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        let toggled: String = line.chars().map(|ch| {
                            if ch.is_ascii_uppercase() { ch.to_ascii_lowercase() }
                            else if ch.is_ascii_lowercase() { ch.to_ascii_uppercase() }
                            else { ch }
                        }).collect();
                        if toggled != line {
                            self.buffer.set_line_content(line_idx, &toggled);
                        }
                    }
                }
            } else {
                for line_idx in start.line_idx..=end.line_idx {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        let s = if line_idx == start.line_idx {
                            self.buffer.grapheme_to_byte(line_idx, start.grapheme_idx)
                        } else { 0 };
                        let e = if line_idx == end.line_idx {
                            self.buffer.grapheme_to_byte(line_idx, end.grapheme_idx)
                        } else { line.len() };
                        if s < e && e <= line.len() {
                            let prefix: String = line[..s].chars().collect();
                            let selected: String = line[s..e].chars().map(|ch| {
                                if ch.is_ascii_uppercase() { ch.to_ascii_lowercase() }
                                else if ch.is_ascii_lowercase() { ch.to_ascii_uppercase() }
                                else { ch }
                            }).collect();
                            let suffix: String = line[e..].chars().collect();
                            let toggled = format!("{prefix}{selected}{suffix}");
                            if toggled != line {
                                self.buffer.set_line_content(line_idx, &toggled);
                            }
                        }
                    }
                }
            }
        }
        self.clear_selection();
        self.set_needs_redraw(true);
    }

    /// Replace visual selection with yank register content.
    fn replace_selection_with_yank(&mut self) {
        if self.yank_register.is_some() {
            // Delete selection first (replaces yank register, so save it)
            let saved_yank = self.yank_register.take();
            let saved_linewise = self.yank_register_linewise;
            self.delete_selection();
            self.yank_register = saved_yank;
            self.yank_register_linewise = saved_linewise;
            // Paste the yank content at current position
            self.paste();
        } else {
            self.delete_selection();
        }
    }

    /// Jump to the matching bracket ( ) [ ] { } < >.
    fn move_to_matching_bracket(&mut self) {
        const PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];
        if let Some(line) = self.buffer.get_line_content(self.text_location.line_idx) {
            let byte_idx = self.buffer.grapheme_to_byte(self.text_location.line_idx, self.text_location.grapheme_idx);
            if let Some(&ch) = line.as_bytes().get(byte_idx) {
                let ch = ch as char;
                let (open, close, forward) = if PAIRS.iter().any(|&(o, _)| o == ch) {
                    (ch, PAIRS.iter().find(|&&(o, _)| o == ch).map(|&(_, c)| c).unwrap_or(ch), true)
                } else if PAIRS.iter().any(|&(_, c)| c == ch) {
                    (PAIRS.iter().find(|&&(_, c)| c == ch).map(|&(o, _)| o).unwrap_or(ch), ch, false)
                } else {
                    return;
                };
                self.buffer.find_matching_bracket(self.text_location, open, close, forward)
                    .map(|loc| {
                        self.text_location = loc;
                        self.snap_to_valid_grapheme();
                    });
            }
        }
    }

    /// Get the word under the cursor for * / # search.
    pub fn get_word_under_cursor(&mut self) -> Option<String> {
        // Move to current word start, then extract to next word start
        let saved = self.text_location;
        self.move_to_current_or_next_word_end(WordSeparatorType::Symbol);

        // Find the start and end of current word
        let start = self.buffer.current_or_prev_word_location_start(&saved, WordSeparatorType::Symbol);
        // Then find the end
        let end = self.buffer.current_or_next_word_location_end(&saved, WordSeparatorType::Symbol);

        self.text_location = saved;
        if start.line_idx == end.line_idx && start.grapheme_idx < end.grapheme_idx {
            if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                let s = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                let e = self.buffer.grapheme_to_byte(end.line_idx, end.grapheme_idx);
                if s < e && e <= line.len() {
                    return Some(line[s..e].to_string());
                }
            }
        }
        None
    }

    /// Swap the selection anchor and cursor (o key in Visual mode).
    pub fn swap_selection_ends(&mut self) {
        if let Some(anchor) = self.selection_anchor {
            let cursor = self.text_location;
            self.selection_anchor = Some(cursor);
            self.text_location = anchor;
            self.snap_to_valid_grapheme();
            self.set_needs_redraw(true);
        }
    }

    /// Yank the inner word at cursor (yiw). Cursor moves to word start.
    pub fn yank_inner_word(&mut self) {
        if let Some((start, end)) = self.buffer.inner_word_range(&self.text_location) {
            if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                let sb = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                let eb = self.buffer.grapheme_to_byte(end.line_idx, end.grapheme_idx);
                if sb < eb && eb <= line.len() {
                    self.yank_register = Some(line[sb..eb].to_string());
                    self.yank_register_linewise = false;
                    self.text_location = start;
                    self.snap_to_valid_grapheme();
                    self.set_needs_redraw(true);
                }
            }
        }
    }

    /// Select the inner word at cursor (viw). Cursor moves to word end.
    pub fn select_inner_word(&mut self) {
        if let Some((start, end)) = self.buffer.inner_word_range(&self.text_location) {
            self.selection_anchor = Some(start);
            // Move cursor to word end - 1 (last char, vi behavior)
            let last = if end.grapheme_idx > 0 {
                Location { line_idx: end.line_idx, grapheme_idx: end.grapheme_idx.saturating_sub(1) }
            } else {
                start
            };
            self.text_location = last;
            self.snap_to_valid_grapheme();
            self.set_needs_redraw(true);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_linewise = false;
    }

    fn selection_range(&self) -> Option<(Location, Location)> {
        let anchor = self.selection_anchor?;
        let cursor = self.text_location;
        if self.selection_linewise {
            let (a, c) = if anchor.line_idx <= cursor.line_idx {
                (anchor, cursor)
            } else {
                (cursor, anchor)
            };
            Some((
                Location { line_idx: a.line_idx, grapheme_idx: 0 },
                Location { line_idx: c.line_idx, grapheme_idx: self.buffer.grapheme_count(c.line_idx) },
            ))
        } else if anchor.line_idx == cursor.line_idx && anchor.grapheme_idx == cursor.grapheme_idx {
            // Same position: cover exactly one grapheme at cursor
            let gc = self.buffer.grapheme_count(cursor.line_idx);
            let end_g = (cursor.grapheme_idx.saturating_add(1)).min(gc);
            Some((cursor, Location { line_idx: cursor.line_idx, grapheme_idx: end_g }))
        } else if anchor.line_idx < cursor.line_idx
            || (anchor.line_idx == cursor.line_idx && anchor.grapheme_idx < cursor.grapheme_idx)
        {
            // Forward: end = cursor+1 (inclusive of cursor's character)
            let gc = self.buffer.grapheme_count(cursor.line_idx);
            let end_g = (cursor.grapheme_idx.saturating_add(1)).min(gc);
            Some((anchor, Location { line_idx: cursor.line_idx, grapheme_idx: end_g }))
        } else {
            // Backward: end = anchor+1 (inclusive of anchor's character)
            let gc = self.buffer.grapheme_count(anchor.line_idx);
            let end_g = (anchor.grapheme_idx.saturating_add(1)).min(gc);
            Some((cursor, Location { line_idx: anchor.line_idx, grapheme_idx: end_g }))
        }
    }

    fn snap_to_valid_grapheme(&mut self) {
        self.text_location.grapheme_idx = min(self.text_location.grapheme_idx, self.buffer.grapheme_count(self.text_location.line_idx));
        if matches!(self.mode, Mode::Normal) &&
            self.buffer.grapheme_count(self.text_location.line_idx) == self.text_location.grapheme_idx {
            self.move_left();
        }
    }

    fn snap_to_valid_line(&mut self) {
        self.text_location.line_idx = min(self.text_location.line_idx, self.buffer.height().saturating_sub(1));
    }

    fn scroll_text_location_into_view(&mut self) {
        let Position { row, col} = self.text_location_to_position();
        self.scroll_vertically(row);
        self.scroll_horizontally(col);
    }

    fn text_location_to_position(&self) -> Position {
        let row = self.text_location.line_idx;
        debug_assert!(row.saturating_sub(1) <= self.buffer.height());
        let col = self.buffer.width_until(row, self.text_location.grapheme_idx);
        Position { row, col }
    }

    fn render_line(at: RowIdx, line_text: &str) -> Result<(), Error> {
        Terminal::print_row(at, line_text)
    }

    pub fn load(&mut self, filename: &str) -> Result<(), Error> {
        let buffer = Buffer::load(filename)?;
        self.buffer = buffer;
        self.text_location = Location::default();
        self.scroll_offset = Position::default();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.yank_register = None;
        self.last_search_query = None;
        self.set_needs_redraw(true);
        Ok(())
    }

    fn command_load(&mut self, filename: &str) -> String {
        let result = self.load(filename);
        if result.is_ok() {
            self.text_location = Location::default();
            format!("Open file {filename:?} successfully")
        } else {
            format!("Fail to open file: {filename:?}, cause by: {}", result.unwrap_err().to_string())
        }
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.buffer.save()?;
        self.set_needs_redraw(true);
        Ok(())
    }

    fn command_save(&mut self) -> String {
        let result = self.save();
        if result.is_ok() {
            "Save file successfully".to_string()
        } else {
            format!("Fail to save file, cause by: {}", result.unwrap_err().to_string())
        }
    }

    pub fn save_as(&mut self, filename: &str) -> Result<(), Error> {
        self.buffer.save_as(filename)?;
        self.set_needs_redraw(true);
        Ok(())
    }

    fn scroll_vertically(&mut self, to: RowIdx) {
        let Size { height, .. } = self.size;
        let offset_changed = if to < self.scroll_offset.row {
            self.scroll_offset.row = to;
            true
        } else if to >= self.scroll_offset.row.saturating_add(height) {
            self.scroll_offset.row = to.saturating_sub(height).saturating_add(1);
            true
        } else {
            false
        };
        if offset_changed {
            self.set_needs_redraw(true);
        }
    }

    fn scroll_horizontally(&mut self, to: ColIdx) {
        let Size { width, .. } = self.size;
        let offset_changed = if to < self.scroll_offset.col {
            self.scroll_offset.col = to;
            true
        } else if to >= self.scroll_offset.col.saturating_add(width) {
            self.scroll_offset.col = to.saturating_sub(width).saturating_add(1);
            true
        } else {
            false
        };
        if offset_changed {
            self.set_needs_redraw(true);
        }
    }

    pub fn caret_position(&self) -> Position {
        self.text_location_to_position()
            .saturating_sub(self.scroll_offset)
    }

    pub fn enter_search(&mut self) {
        self.search_info = Some(SearchInfo {
            prev_location: self.text_location,
            prev_scroll_offset: self.scroll_offset,
            query: None
        })
    }

    pub fn set_search_backward(&mut self) {
        self.last_search_direction = SearchDirection::Backward;
    }

    pub fn exit_search(&mut self) {
        self.search_info = None;
        self.set_needs_redraw(true);
    }

    pub fn confirm_search(&mut self) {
        if let Some(ref search_info) = self.search_info {
            self.last_search_query = search_info.query.clone();
        }
        self.exit_search();
    }

    pub fn exit_command_mode(&mut self) {
        self.command_info = None;
        self.set_needs_redraw(true);
    }

    pub fn dismiss_search(&mut self) {
        if let Some(search_info) = &self.search_info { 
            self.text_location = search_info.prev_location;
            self.scroll_offset = search_info.prev_scroll_offset;
            self.scroll_text_location_into_view();
        }
        self.exit_search();
    }

    pub fn dismiss_command_mode(&mut self) {
        self.scroll_text_location_into_view();
        self.exit_command_mode();
    }
    
    pub fn search(&mut self, query: &str) {
        if let Some(search_info) = &mut self.search_info {
            search_info.query = Some(Line::from(query));
        }
        self.search_in_direction(self.text_location, SearchDirection::default());
    }

    pub fn execute_command(&mut self, command_value: CommandValue) -> Option<String> {
        self.dismiss_command_mode();
        match command_value {
            CommandValue::Quit => {
                if self.get_status().is_modified {
                    Some("Change unsaved, Please save before quit or use \"q!\" force quit".to_string())
                } else {
                    None
                }
            }
            CommandValue::ForceQuit => None,
            CommandValue::Save => {
                if self.is_file_loaded() {
                    Some(self.command_save())
                } else {
                    Some("No filename provided!".to_string())
                }
            }
            CommandValue::SaveAs(filename) => {
                let result = self.save_as(filename.as_str());
                if result.is_ok() {
                    Some(format!("Save as file: {filename:?} successfully"))
                } else {
                    Some(format!("Fail to save as file: {filename:?}, cause by: {}", result.unwrap_err().to_string()))
                }
            }
            CommandValue::SaveAndQuit => {
                if !self.is_file_loaded() {
                    return Some("No filename provided!".to_string());
                }
                let result = self.save();
                if result.is_err() {
                    return Some(format!("Fail to save file, cause by: {}", result.unwrap_err().to_string()));
                }
                None
            }
            CommandValue::SaveAsAndQuit(filename) => {
                let result = self.save_as(filename.as_str());
                if result.is_err() {
                    return Some(format!("Fail to save as file: {filename:?}, cause by: {}", result.unwrap_err().to_string()))
                }
                None
            }
            CommandValue::Open(filename) => {
                if self.is_file_loaded() && self.get_status().is_modified {
                    Some("Current file unsaved, please save this file before opening a another file or use \"e! your_filename\" discard current change".to_string())
                } else if self.get_status().is_modified {
                    Some("Current change unsaved, consider save this change as a new file before opening a another file or use \"e! your_filename\" discard current change".to_string())
                } else {
                    Some(self.command_load(filename.as_str()))
                }
            }
            CommandValue::ForceOpen(filename) => {
                Some(self.command_load(filename.as_str()))
            }
            CommandValue::NoHighlight => {
                self.search_info = None;
                self.set_needs_redraw(true);
                None
            }
            CommandValue::SetLineNumbers(show) => {
                crate::editor::terminal::Terminal::set_show_line_numbers(show);
                self.set_needs_redraw(true);
                Some(format!("Line numbers {}", if show { "enabled" } else { "disabled" }))
            }
        }
    }

    pub fn search_next(&mut self) {
        let step_right = self.get_search_query()
            .map_or(1, |query| min(query.grapheme_count(), 1));
        let location = Location {
            line_idx: self.text_location.line_idx,
            grapheme_idx: self.text_location.grapheme_idx.saturating_add(step_right)
        };
        self.search_in_direction(location, SearchDirection::Forward);
    }
    
    pub fn search_prev(&mut self) {
        self.search_in_direction(self.text_location, SearchDirection::Backward);
    }

    fn get_search_query(&self) -> Option<&Line> {
        let query = self.search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref());
        debug_assert!(query.is_some(), "Attempting to search without malformed search info present");
        query
    }

    fn search_in_direction(&mut self, from: Location, direction: SearchDirection) {
        if let Some(location) = self.get_search_query().and_then(|query| {
            if query.is_empty() {
                None
            } else if direction == SearchDirection::Forward {
                self.buffer.search_forward(query, from)
            } else {
                self.buffer.search_backward(query, from)
            }
        }) {
            self.text_location = location;
            self.center_text_location();
        } else {
            self.set_needs_redraw(true);
        }
    }

    fn insert_char(&mut self, character: char) {
        let old_len = self.buffer.grapheme_count(self.text_location.line_idx);
        self.buffer.insert_char(character, self.text_location);
        let new_len = self.buffer.grapheme_count(self.text_location.line_idx);
        if new_len.saturating_sub(old_len) > 0 {
            self.handle_move_command(Move::Right);
        }
        self.set_needs_redraw(true);
    }

    fn delete_backward(&mut self) {
        if self.text_location.grapheme_idx != 0 {
            self.handle_move_command(Move::Left);
        } else {
            if self.text_location.line_idx == 0 {
                return;
            }
            self.handle_move_command(Move::Up);
            self.handle_move_command(Move::EndOfLine);
        }
        self.delete();
    }

    fn delete(&mut self) {
        self.buffer.delete(self.text_location);
        self.snap_to_valid_grapheme();
        self.set_needs_redraw(true);
    }

    fn insert_newline(&mut self) {
        self.buffer.insert_newline(self.text_location);
        self.handle_move_command(Move::StartOfLine);
        self.handle_move_command(Move::Down);
        self.set_needs_redraw(true);
    }

    fn move_to_first_line(&mut self) {
        self.text_location.grapheme_idx = 0;
        self.text_location.line_idx = 0;
    }

    fn delete_line(&mut self) {
        self.take_snapshot();
        let line_idx = self.text_location.line_idx;
        let line_content = self.buffer.delete_line(line_idx);
        if let Some(text) = line_content {
            self.yank_register = Some(text);
            self.yank_register_linewise = true;
        }
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
        self.move_to_start_of_line();
        self.set_needs_redraw(true);
    }

    fn replace_char(&mut self, ch: char) {
        self.take_snapshot();
        let grapheme_count = self.buffer.grapheme_count(self.text_location.line_idx);
        if matches!(self.mode, Mode::Normal) && grapheme_count > 0
            && self.text_location.grapheme_idx < grapheme_count
        {
            self.delete();
            self.buffer.insert_char(ch, self.text_location);
            self.set_needs_redraw(true);
        } else if matches!(self.mode, Mode::Insert) && self.text_location.grapheme_idx < grapheme_count {
            self.delete();
            self.buffer.insert_char(ch, self.text_location);
            self.set_needs_redraw(true);
        }
    }

    fn yank_line(&mut self) {
        if let Some(line) = self.buffer.get_line_content(self.text_location.line_idx) {
            self.yank_register = Some(line);
            self.yank_register_linewise = true;
        }
    }

    fn paste(&mut self) {
        self.take_snapshot();
        if let Some(text) = self.yank_register.clone() {
            if self.yank_register_linewise {
                if self.text_location.line_idx == self.buffer.last_line_idx() {
                    // On the last line: move past end so insert_newline appends a new line
                    self.move_to_end_of_line();
                    self.text_location.line_idx = self.buffer.height();
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.move_to_start_of_line();
                    self.handle_move_command(Move::Down);
                }
                self.buffer.insert_newline(self.text_location);
            } else {
                // Non-linewise: paste AFTER cursor
                self.text_location.grapheme_idx += 1;
            }
            for ch in text.chars() {
                if ch == '\n' {
                    self.buffer.insert_newline(self.text_location);
                    self.text_location.line_idx += 1;
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.buffer.insert_char(ch, self.text_location);
                    self.text_location.grapheme_idx += 1;
                }
            }
            self.snap_to_valid_grapheme();
            self.snap_to_valid_line();
            self.set_needs_redraw(true);
        }
    }

    fn paste_above(&mut self) {
        self.take_snapshot();
        if let Some(text) = self.yank_register.clone() {
            if self.yank_register_linewise {
                self.move_to_start_of_line();
                self.buffer.insert_newline(self.text_location);
            }
            for ch in text.chars() {
                if ch == '\n' {
                    self.buffer.insert_newline(self.text_location);
                    self.text_location.line_idx += 1;
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.buffer.insert_char(ch, self.text_location);
                    self.text_location.grapheme_idx += 1;
                }
            }
            self.snap_to_valid_grapheme();
            self.snap_to_valid_line();
            self.set_needs_redraw(true);
        }
    }

    fn delete_to_next_word_start(&mut self) {
        self.take_snapshot();
        let line_idx = self.text_location.line_idx;
        let grapheme_count = self.buffer.grapheme_count(line_idx);
        // Check if there is a next word start on the current line.
        // Using the line-level method avoids the cross-line fallback from
        // next_word_location_start (which returns grapheme_count-1 when no next word exists).
        let count = if let Some(target) = self.buffer.find_next_word_start_on_line(line_idx, self.text_location.grapheme_idx) {
            target.saturating_sub(self.text_location.grapheme_idx)
        } else {
            grapheme_count.saturating_sub(self.text_location.grapheme_idx)
        };
        for _ in 0..count {
            self.delete();
        }
        self.set_needs_redraw(true);
    }
    
    /// Whether a Move command should be confined to the current line
    /// (word-level and character-level motions are same-line only).
    fn is_same_line_motion(cmd: Move) -> bool {
        matches!(cmd,
            Move::Left | Move::Right
            | Move::NextWordSplitBySymbol | Move::NextWordSplitByWhitespace
            | Move::EndOfWordSplitBySymbol | Move::EndOfWordSplitByWhitespace
            | Move::PrevWordSplitBySymbol | Move::PrevWordSplitByWhitespace
            | Move::StartOfLine | Move::EndOfLine | Move::FirstVisibleCharOfLine
            | Move::FindChar(_) | Move::FindCharBackward(_)
            | Move::TillChar(_) | Move::TillCharBackward(_)
            | Move::RepeatFindSameDir | Move::RepeatFindOppositeDir
        )
    }

    /// Cap a location that crossed lines back to the end of the current line.
    fn cap_same_line_end(&self, start: Location, end: Location) -> Location {
        if end.line_idx != start.line_idx {
            Location {
                line_idx: start.line_idx,
                grapheme_idx: self.buffer.grapheme_count(start.line_idx),
            }
        } else {
            end
        }
    }

    /// Delete from cursor to the beginning of the current line (Ctrl+U in Insert mode).
    fn delete_to_line_start(&mut self) {
        self.take_snapshot();
        let count = self.text_location.grapheme_idx;
        for _ in 0..count {
            self.delete_backward();
        }
        self.set_needs_redraw(true);
    }

    /// Delete text from cursor to the destination of a motion (d + motion).
    fn delete_range_to_motion(&mut self, move_cmd: Move) {
        self.take_snapshot();
        let start = self.text_location;
        let same_line = Self::is_same_line_motion(move_cmd);
        self.handle_move_command(move_cmd);
        let end = if same_line { self.cap_same_line_end(start, self.text_location) } else { self.text_location };
        self.delete_range_inner(start, end);
        self.set_needs_redraw(true);
    }

    /// Yank text from cursor to the destination of a motion (y + motion).
    fn yank_range_to_motion(&mut self, move_cmd: Move) {
        let start = self.text_location;
        let same_line = Self::is_same_line_motion(move_cmd);
        let text = self.extract_text_range(start, move_cmd);
        if let Some(t) = text {
            // If motion crossed lines but is same-line-only, strip trailing newline
            let final_text = if same_line && t.contains('\n') {
                // Take only the portion before the first newline
                t.split('\n').next().unwrap_or("").to_string()
            } else {
                t
            };
            if !final_text.is_empty() {
                self.yank_register = Some(final_text);
                self.yank_register_linewise = false;
            }
        }
        self.set_needs_redraw(true);
    }

    /// Delete text from cursor to the destination of a motion, then enter Insert (c + motion).
    fn change_range_to_motion(&mut self, move_cmd: Move) {
        self.take_snapshot();
        let start = self.text_location;
        let same_line = Self::is_same_line_motion(move_cmd);
        self.handle_move_command(move_cmd);
        let end = if same_line { self.cap_same_line_end(start, self.text_location) } else { self.text_location };
        self.delete_range_inner(start, end);
        self.set_needs_redraw(true);
    }

    /// Core range deletion: delete all characters between start (inclusive) and end (exclusive).
    fn delete_range_inner(&mut self, start: Location, end: Location) {
        if start.line_idx == end.line_idx {
            // Same line: delete count characters at start position
            let count = end.grapheme_idx.saturating_sub(start.grapheme_idx);
            self.text_location = start;
            for _ in 0..count {
                self.delete();
            }
        } else if start.line_idx < end.line_idx {
            // Cross-line forward (e.g. d+j, d+Down)
            // Delete from start of end line to end.grapheme_idx first
            let end_prefix = end.grapheme_idx;
            self.text_location = Location { line_idx: end.line_idx, grapheme_idx: 0 };
            for _ in 0..end_prefix {
                self.delete();
            }
            // Delete intermediate lines (from start.line_idx+1 to end.line_idx)
            // After deleting end-line prefix, line indices may shift.
            // Work backwards from end to start.
            for _ in start.line_idx.saturating_add(1)..end.line_idx {
                self.text_location = Location { line_idx: start.line_idx.saturating_add(1), grapheme_idx: 0 };
                self.delete_line();
            }
            // Delete start line suffix from start.grapheme_idx to end of line
            self.text_location = start;
            let grapheme_count = self.buffer.grapheme_count(start.line_idx);
            let count = grapheme_count.saturating_sub(start.grapheme_idx);
            for _ in 0..count {
                self.delete();
            }
        } else {
            // Cross-line backward (e.g. d+k, d+Up)
            // Delete from start line suffix
            self.text_location = start;
            let grapheme_count = self.buffer.grapheme_count(start.line_idx);
            let count = grapheme_count.saturating_sub(start.grapheme_idx);
            for _ in 0..count {
                self.delete();
            }
            // Delete intermediate lines (only lines strictly between end and start)
            for _ in end.line_idx.saturating_add(1)..start.line_idx {
                self.text_location = Location { line_idx: end.line_idx, grapheme_idx: 0 };
                self.delete_line();
            }
            // Delete end line prefix
            let end_prefix = end.grapheme_idx;
            self.text_location = Location { line_idx: end.line_idx, grapheme_idx: 0 };
            for _ in 0..end_prefix {
                self.delete();
            }
        }
    }

    /// Extract text from start position to the destination of a motion without deleting.
    fn extract_text_range(&mut self, start: Location, move_cmd: Move) -> Option<String> {
        // Save current state
        let saved_location = self.text_location;
        // Execute the motion to get the end position
        self.handle_move_command(move_cmd);
        let end = self.text_location;
        // Restore cursor
        self.text_location = saved_location;

        let mut text = String::new();
        if start.line_idx == end.line_idx {
            if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                let start_byte = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                let end_byte = self.buffer.grapheme_to_byte(start.line_idx, end.grapheme_idx);
                if start_byte < end_byte && end_byte <= line.len() {
                    text.push_str(&line[start_byte..end_byte]);
                }
            }
        } else {
            // Cross-line: collect each line's portion
            if start.line_idx < end.line_idx {
                // First line: from start to end of line
                if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                    let start_byte = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                    if start_byte < line.len() {
                        text.push_str(&line[start_byte..]);
                    }
                    text.push('\n');
                }
                // Intermediate lines
                for line_idx in start.line_idx.saturating_add(1)..end.line_idx {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                // Last line: from start to end.grapheme_idx
                if let Some(line) = self.buffer.get_line_content(end.line_idx) {
                    let end_byte = self.buffer.grapheme_to_byte(end.line_idx, end.grapheme_idx);
                    if end_byte > 0 && end_byte <= line.len() {
                        text.push_str(&line[..end_byte]);
                    }
                }
            } else if start.line_idx > end.line_idx {
                // Backward cross-line (e.g. y+k): start > end
                // End line (lower index): from end.grapheme_idx to end of line
                if let Some(line) = self.buffer.get_line_content(end.line_idx) {
                    let end_byte = self.buffer.grapheme_to_byte(end.line_idx, end.grapheme_idx);
                    if end_byte < line.len() {
                        text.push_str(&line[end_byte..]);
                    }
                    text.push('\n');
                }
                // Intermediate lines
                for line_idx in end.line_idx.saturating_add(1)..start.line_idx {
                    if let Some(line) = self.buffer.get_line_content(line_idx) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                // Start line (higher index): from 0 to start.grapheme_idx
                if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                    let start_byte = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                    if start_byte > 0 && start_byte <= line.len() {
                        text.push_str(&line[..start_byte]);
                    }
                }
            }
        }
        if text.is_empty() { None } else { Some(text) }
    }

    pub fn take_snapshot(&mut self) {
        self.redo_stack.clear();
        let snapshot = BufferSnapshot {
            lines: self.buffer.get_all_lines(),
            cursor: self.text_location,
            scroll_offset: self.scroll_offset,
        };
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = BufferSnapshot {
                lines: self.buffer.get_all_lines(),
                cursor: self.text_location,
                scroll_offset: self.scroll_offset,
            };
            self.redo_stack.push(current);
            self.buffer.set_lines(&snapshot.lines);
            self.text_location = snapshot.cursor;
            self.scroll_offset = snapshot.scroll_offset;
            self.snap_to_valid_grapheme();
            self.snap_to_valid_line();
            self.scroll_text_location_into_view();
            self.set_needs_redraw(true);
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = BufferSnapshot {
                lines: self.buffer.get_all_lines(),
                cursor: self.text_location,
                scroll_offset: self.scroll_offset,
            };
            self.undo_stack.push(current);
            self.buffer.set_lines(&snapshot.lines);
            self.text_location = snapshot.cursor;
            self.scroll_offset = snapshot.scroll_offset;
            self.snap_to_valid_grapheme();
            self.snap_to_valid_line();
            self.scroll_text_location_into_view();
            self.set_needs_redraw(true);
        }
    }

    fn repeat_search(&mut self, direction: SearchDirection) {
        if let Some(query) = self.last_search_query.clone() {
            if query.is_empty() {
                return;
            }
            if direction == SearchDirection::Forward {
                let step_right = min(query.grapheme_count(), 1);
                let location = Location {
                    line_idx: self.text_location.line_idx,
                    grapheme_idx: self.text_location.grapheme_idx.saturating_add(step_right),
                };
                self.search_in_direction(location, direction);
            } else {
                self.search_in_direction(self.text_location, direction);
            }
        }
    }

    fn center_text_location(&mut self) {
        let Size { height, width } = self.size;
        let Position { row, col } = self.text_location_to_position();
        let vertical_mid = height / 2;
        let horizontal_mid = width / 2;
        self.scroll_offset.row = row.saturating_sub(vertical_mid);
        self.scroll_offset.col = col.saturating_sub(horizontal_mid);
        self.set_needs_redraw(true);
    }
}

impl UIComponent for View {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
        self.scroll_text_location_into_view();
    }

    fn draw(&mut self, origin_row: RowIdx) -> Result<(), Error> {
        let Size { height, width } = self.size;
        let end_y = origin_row.saturating_add(height);
        let scroll_top = self.scroll_offset.row;

        Terminal::set_cursor_line(self.text_location.line_idx);
        let query = self.search_info.as_ref()
            .and_then(|search_info| search_info.query.as_deref());
        let selected_match = query.is_some().then_some(self.text_location);
        let mut highlighter = Highlighter::new(query, selected_match, self.buffer.get_file_info().get_file_type());

        for current_row in 0..end_y.saturating_add(scroll_top) {
            self.buffer.highlight(current_row, &mut highlighter);
        }

        let sel_range = self.selection_range();

        for current_row in origin_row..end_y {
            let line_idx = current_row.saturating_sub(origin_row).saturating_add(scroll_top);
            let left = self.scroll_offset.col;
            let right = self.scroll_offset.col.saturating_add(width);
            if let Some(mut annotated_string) = self.buffer.get_highlight_substring(line_idx, left..right, &highlighter) {
                // Add selection highlight for lines within the visual selection range
                if let Some((sel_start, sel_end)) = sel_range {
                    if line_idx >= sel_start.line_idx && line_idx <= sel_end.line_idx {
                        let scroll_byte = self.buffer.column_to_byte(line_idx, left);
                        let start_g = if line_idx == sel_start.line_idx { sel_start.grapheme_idx } else { 0 };
                        let raw_start = self.buffer.grapheme_to_byte(line_idx, start_g);
                        let raw_end = if self.selection_linewise || line_idx < sel_end.line_idx {
                            self.buffer.get_line_content(line_idx).map_or(0, |l| l.len())
                        } else {
                            self.buffer.grapheme_to_byte(line_idx, sel_end.grapheme_idx)
                        };
                        // Adjust for horizontal scroll and cap to visible string bounds
                        let adj_start = raw_start.saturating_sub(scroll_byte);
                        let adj_end = raw_end.saturating_sub(scroll_byte);
                        // Ensure annotation is within the clipped string
                        if adj_start < adj_end {
                            let max_byte = annotated_string.byte_len();
                            if adj_start < max_byte {
                                annotated_string.add_annotation(
                                    AnnotationType::Selection,
                                    adj_start,
                                    adj_end.min(max_byte),
                                );
                            }
                        }
                    }
                }
                Terminal::print_annotated_row(current_row, line_idx, &annotated_string)?
            } else {
                Self::render_line(current_row, "")?;
            }
        }
        Ok(())
    }
}