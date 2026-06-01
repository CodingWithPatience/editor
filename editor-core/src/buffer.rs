use crate::annotated_string::AnnotatedString;
use crate::file_info::FileInfo;
use crate::highlighter::Highlighter;
use crate::line::Line;
use crate::prelude::{ColIdx, GraphemeIdx, LineIdx, Location, WordSeparatorType};
use std::fs::{read_to_string, File};
use std::io::Error;
use std::io::Write;
use std::ops::Range;

#[derive(Default)]
pub struct Buffer {
    lines: Vec<Line>,
    file_info: FileInfo,
    dirty: bool,
}

impl Buffer {
    pub fn current_or_prev_word_location_start(
        &self,
        location: &Location,
        separator_type: WordSeparatorType,
    ) -> Location {
        for i in (0..=location.line_idx).rev() {
            let mut start_at_next = false;
            let grapheme_idx_start = if location.line_idx == i {
                location.grapheme_idx
            } else {
                start_at_next = true;
                self.lines
                    .get(i)
                    .map_or(0, |line| line.grapheme_count().saturating_sub(1))
            };
            let grapheme_idx = self.lines.get(i).map_or(None, |line| {
                line.current_or_next_word_end_grapheme_idx(
                    grapheme_idx_start,
                    separator_type,
                    start_at_next,
                    true,
                )
            });
            if let Some(g) = grapheme_idx {
                return Location {
                    line_idx: i,
                    grapheme_idx: g,
                };
            }
        }
        Location {
            line_idx: 0,
            grapheme_idx: 0,
        }
    }

    pub fn current_or_next_word_location_end(
        &self,
        location: &Location,
        separator_type: WordSeparatorType,
    ) -> Location {
        for i in location.line_idx..self.lines.len() {
            let mut start_at_prev = false;
            let grapheme_idx_start = if location.line_idx == i {
                location.grapheme_idx
            } else {
                start_at_prev = true;
                0
            };
            let grapheme_idx = self.lines.get(i).map_or(None, |line| {
                line.current_or_next_word_end_grapheme_idx(
                    grapheme_idx_start,
                    separator_type,
                    start_at_prev,
                    false,
                )
            });
            if let Some(g) = grapheme_idx {
                return Location {
                    line_idx: i,
                    grapheme_idx: g,
                };
            }
        }
        let line_idx = self.lines.len().saturating_sub(1);
        Location {
            line_idx,
            grapheme_idx: self
                .lines
                .get(line_idx)
                .map_or(0, |line| line.grapheme_count().saturating_sub(1)),
        }
    }

    pub fn next_word_location_start(
        &self,
        location: &Location,
        separator_type: WordSeparatorType,
    ) -> Location {
        for i in location.line_idx..self.lines.len() {
            let mut start_at_prev = false;
            let grapheme_idx_start = if location.line_idx == i {
                location.grapheme_idx
            } else {
                start_at_prev = true;
                0
            };
            let grapheme_idx = self.lines.get(i).map_or(None, |line| {
                line.next_word_start_grapheme_idx(grapheme_idx_start, separator_type, start_at_prev)
            });
            if let Some(g) = grapheme_idx {
                return Location {
                    line_idx: i,
                    grapheme_idx: g,
                };
            }
        }
        let line_idx = self.lines.len().saturating_sub(1);
        Location {
            line_idx,
            grapheme_idx: self
                .lines
                .get(line_idx)
                .map_or(0, |line| line.grapheme_count().saturating_sub(1)),
        }
    }

    pub fn last_line_idx(&self) -> LineIdx {
        self.lines.len().saturating_sub(1)
    }

    pub fn first_visible_of_line_grapheme_idx(&self, line_idx: LineIdx) -> GraphemeIdx {
        self.lines
            .get(line_idx)
            .map_or(0, |line| line.first_visible_grapheme_idx())
    }

    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub const fn get_file_info(&self) -> &FileInfo {
        &self.file_info
    }

    pub fn grapheme_count(&self, line_idx: LineIdx) -> GraphemeIdx {
        self.lines.get(line_idx).map_or(0, Line::grapheme_count)
    }

    pub fn width_until(&self, line_idx: LineIdx, until: GraphemeIdx) -> GraphemeIdx {
        self.lines
            .get(line_idx)
            .map_or(0, |line| line.width_until(until))
    }

    pub fn get_highlight_substring(
        &self,
        line_idx: LineIdx,
        range: Range<GraphemeIdx>,
        highlighter: &Highlighter,
    ) -> Option<AnnotatedString> {
        self.lines.get(line_idx).map(|line| {
            line.get_annotated_visible_substr(range, Some(&highlighter.get_annotations(line_idx)))
        })
    }

    pub fn highlight(&self, line_idx: LineIdx, highlighter: &mut Highlighter) {
        if let Some(line) = self.lines.get(line_idx) {
            highlighter.highlight(line_idx, line);
        }
    }

    pub fn load(filename: &str) -> Result<Self, Error> {
        let contents = read_to_string(filename)?;
        let mut lines = Vec::new();
        for content_line in contents.lines() {
            lines.push(Line::from(content_line));
        }
        Ok(Self {
            lines,
            file_info: FileInfo::from(filename),
            dirty: false,
        })
    }

    fn save_to_file(&self, file_info: &FileInfo) -> Result<(), Error> {
        if let Some(file_path) = &file_info.get_path() {
            let mut file = File::create(file_path)?;
            for line in &self.lines {
                writeln!(file, "{line}")?;
            }
        } else {
            #[cfg(debug_assertions)]
            {
                panic!("Attempting to save with no file path present");
            }
        }
        Ok(())
    }

    pub fn save_as(&mut self, filename: &str) -> Result<(), Error> {
        let file_info = FileInfo::from(filename);
        self.save_to_file(&file_info)?;
        self.file_info = file_info;
        self.dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.save_to_file(&self.file_info)?;
        self.dirty = false;
        Ok(())
    }

    pub const fn is_file_loaded(&self) -> bool {
        self.file_info.has_path()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn height(&self) -> LineIdx {
        self.lines.len()
    }

    pub fn insert_char(&mut self, character: char, at: Location) {
        debug_assert!(at.line_idx <= self.height());
        if at.line_idx == self.height() {
            self.lines.push(Line::from(&character.to_string()));
            self.dirty = true;
        } else if let Some(line) = self.lines.get_mut(at.line_idx) {
            line.insert_char(character, at.grapheme_idx);
            self.dirty = true;
        }
    }

    pub fn delete(&mut self, at: Location) {
        if let Some(line) = self.lines.get(at.line_idx) {
            if at.grapheme_idx >= line.grapheme_count()
                && self.height() > at.line_idx.saturating_add(1)
            {
                let next_line = self.lines.remove(at.line_idx.saturating_add(1));
                #[allow(clippy::indexing_slicing)]
                self.lines[at.line_idx].append(&next_line);
                self.dirty = true;
            } else if at.grapheme_idx < line.grapheme_count() {
                #[allow(clippy::indexing_slicing)]
                self.lines[at.line_idx].delete(at.grapheme_idx);
                self.dirty = true;
            }
        }
    }

    pub fn delete_word_forward(&mut self, at: Location) -> Location {
        let grapheme_count = self.grapheme_count(at.line_idx);
        if at.grapheme_idx >= grapheme_count {
            if self.height() > at.line_idx.saturating_add(1) {
                self.delete(at);
            }
            return at;
        }
        if let Some(line) = self.lines.get_mut(at.line_idx) {
            line.delete_word_forward_from(at.grapheme_idx);
            self.dirty = true;
        }
        at
    }

    pub fn delete_word_backward(&mut self, at: Location) -> Location {
        if at.grapheme_idx == 0 {
            if at.line_idx > 0 {
                let prev_len = self.grapheme_count(at.line_idx.saturating_sub(1));
                let new_at = Location {
                    line_idx: at.line_idx.saturating_sub(1),
                    grapheme_idx: prev_len,
                };
                self.delete(new_at);
                return new_at;
            }
            return at;
        }
        if let Some(line) = self.lines.get_mut(at.line_idx) {
            let new_idx = line.delete_word_backward_from(at.grapheme_idx);
            self.dirty = true;
            return Location {
                line_idx: at.line_idx,
                grapheme_idx: new_idx,
            };
        }
        at
    }

    /// Join the next line onto the end of line at `line_idx`.
    pub fn join_next_line(&mut self, line_idx: LineIdx) {
        if line_idx < self.last_line_idx() {
            if let Some(next_line) = self.lines.get(line_idx.saturating_add(1)) {
                let content = next_line.to_string();
                #[allow(clippy::indexing_slicing)]
                self.lines[line_idx].append_char_str(&content);
            }
            self.lines.remove(line_idx.saturating_add(1));
            self.dirty = true;
        }
    }

    /// Find matching bracket from location. forward=true for opening bracket search.
    pub fn find_matching_bracket(
        &self,
        from: Location,
        open: char,
        close: char,
        forward: bool,
    ) -> Option<Location> {
        let mut depth: i32 = 0;
        if forward {
            // Search forward for the matching closing bracket
            for line_idx in from.line_idx..self.lines.len() {
                if let Some(line) = self.lines.get(line_idx) {
                    let start = if line_idx == from.line_idx {
                        from.grapheme_idx
                            .saturating_add(1)
                            .min(line.grapheme_count())
                    } else {
                        0
                    };
                    for i in start..line.grapheme_count() {
                        let ch = self.grapheme_at(line_idx, i);
                        if ch == open {
                            depth += 1;
                        }
                        if ch == close {
                            if depth == 0 {
                                return Some(Location {
                                    line_idx,
                                    grapheme_idx: i,
                                });
                            }
                            depth -= 1;
                        }
                    }
                }
            }
        } else {
            // Search backward for the matching opening bracket
            for line_idx in (0..=from.line_idx).rev() {
                if let Some(line) = self.lines.get(line_idx) {
                    let end = if line_idx == from.line_idx {
                        from.grapheme_idx.min(line.grapheme_count())
                    } else {
                        line.grapheme_count()
                    };
                    for i in (0..end).rev() {
                        let ch = self.grapheme_at(line_idx, i);
                        if ch == close {
                            depth += 1;
                        }
                        if ch == open {
                            if depth == 0 {
                                return Some(Location {
                                    line_idx,
                                    grapheme_idx: i,
                                });
                            }
                            depth -= 1;
                        }
                    }
                }
            }
        }
        None
    }

    /// Get the character at a specific grapheme index in a line.
    fn grapheme_at(&self, line_idx: LineIdx, grapheme_idx: GraphemeIdx) -> char {
        self.lines
            .get(line_idx)
            .and_then(|line| {
                let byte_idx = line.grapheme_idx_to_byte_idx(grapheme_idx);
                line[byte_idx..].chars().next()
            })
            .unwrap_or('\0')
    }

    pub fn find_char_on_line(
        &self,
        line_idx: LineIdx,
        from: GraphemeIdx,
        ch: char,
        forward: bool,
    ) -> Option<GraphemeIdx> {
        self.lines.get(line_idx).and_then(|line| {
            if forward {
                line.find_char_forward(from, ch)
            } else {
                line.find_char_backward(from, ch)
            }
        })
    }

    /// Return the start and end (exclusive) of the inner word at the given location.
    /// Return the (start, end_exclusive) range of the inner word at the cursor.
    /// Inner word = contiguous non-whitespace graphemes containing the cursor.
    /// Returns None if cursor is on whitespace or at an invalid position.
    pub fn inner_word_range(&self, location: &Location) -> Option<(Location, Location)> {
        let line = self.lines.get(location.line_idx)?;
        let gc = line.grapheme_count();
        if location.grapheme_idx >= gc {
            return None;
        }
        let ch = self.grapheme_at(location.line_idx, location.grapheme_idx);
        if ch == '\0' || ch.is_whitespace() {
            return None;
        }
        // Scan backward to find word start
        let mut start = *location;
        while start.grapheme_idx > 0 {
            let prev = self.grapheme_at(start.line_idx, start.grapheme_idx.saturating_sub(1));
            if prev == '\0' || prev.is_whitespace() {
                break;
            }
            start.grapheme_idx = start.grapheme_idx.saturating_sub(1);
        }
        // Scan forward to find word end (exclusive)
        let mut end = *location;
        while end.grapheme_idx.saturating_add(1) < gc {
            let next = self.grapheme_at(end.line_idx, end.grapheme_idx.saturating_add(1));
            if next == '\0' || next.is_whitespace() {
                break;
            }
            end.grapheme_idx = end.grapheme_idx.saturating_add(1);
        }
        end.grapheme_idx = end.grapheme_idx.saturating_add(1).min(gc);
        if start.grapheme_idx < end.grapheme_idx {
            Some((start, end))
        } else {
            None
        }
    }

    pub fn find_next_word_start_on_line(
        &self,
        line_idx: LineIdx,
        from: GraphemeIdx,
    ) -> Option<GraphemeIdx> {
        self.lines.get(line_idx).and_then(|line| {
            line.next_word_start_grapheme_idx(from, WordSeparatorType::Symbol, false)
        })
    }

    pub fn insert_newline(&mut self, at: Location) {
        if self.is_empty() {
            self.lines.push(Line::default());
            self.dirty = true;
        }
        if at.line_idx == self.height() {
            self.lines.push(Line::default());
            self.dirty = true;
        } else if let Some(line) = self.lines.get_mut(at.line_idx) {
            let new_line = line.split(at.grapheme_idx);
            self.lines.insert(at.line_idx.saturating_add(1), new_line);
            self.dirty = true;
        }
    }

    pub fn delete_line(&mut self, line_idx: LineIdx) -> Option<String> {
        if line_idx < self.lines.len() {
            let deleted = self.lines.remove(line_idx).to_string();
            self.dirty = true;
            if self.lines.is_empty() {
                self.lines.push(Line::default());
            }
            Some(deleted)
        } else {
            None
        }
    }

    pub fn get_all_lines(&self) -> Vec<String> {
        self.lines.iter().map(|l| l.to_string()).collect()
    }

    pub fn set_lines(&mut self, lines: &[String]) {
        self.lines = lines.iter().map(|s| Line::from(s.as_str())).collect();
        self.dirty = true;
    }

    pub fn get_line_content(&self, line_idx: LineIdx) -> Option<String> {
        self.lines.get(line_idx).map(|l| l.to_string())
    }

    pub fn set_line_content(&mut self, line_idx: LineIdx, content: &str) {
        if let Some(line) = self.lines.get_mut(line_idx) {
            *line = Line::from(content);
            self.dirty = true;
        }
    }

    pub fn column_to_byte(&self, line_idx: LineIdx, col: ColIdx) -> usize {
        self.lines
            .get(line_idx)
            .map_or(0, |line| line.column_to_byte(col))
    }

    pub fn grapheme_to_byte(&self, line_idx: LineIdx, grapheme_idx: GraphemeIdx) -> usize {
        self.lines
            .get(line_idx)
            .map_or(0, |line| line.grapheme_idx_to_byte_idx(grapheme_idx))
    }

    pub fn search_forward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }
        let mut is_first = true;
        for (line_idx, line) in self
            .lines
            .iter()
            .enumerate()
            .cycle()
            .skip(from.line_idx)
            .take(self.lines.len().saturating_add(1))
        {
            let from_grapheme_idx = if is_first {
                is_first = false;
                from.grapheme_idx
            } else {
                0
            };
            if let Some(grapheme_idx) = line.search_forward(query, from_grapheme_idx) {
                return Some(Location {
                    grapheme_idx,
                    line_idx,
                });
            }
        }
        None
    }

    pub fn search_backward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }
        let mut is_first = true;
        for (line_idx, line) in self
            .lines
            .iter()
            .enumerate()
            .rev()
            .cycle()
            .skip(
                self.lines
                    .len()
                    .saturating_sub(from.line_idx)
                    .saturating_sub(1),
            )
            .take(self.lines.len().saturating_add(1))
        {
            let from_grapheme_idx = if is_first {
                is_first = false;
                from.grapheme_idx
            } else {
                line.grapheme_count()
            };
            if let Some(grapheme_idx) = line.search_backward(query, from_grapheme_idx) {
                return Some(Location {
                    grapheme_idx,
                    line_idx,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{Location, WordSeparatorType};

    fn loc(line_idx: usize, grapheme_idx: usize) -> Location {
        Location {
            line_idx,
            grapheme_idx,
        }
    }

    // --- 默认状态 ---

    #[test]
    fn default_buffer() {
        let buf = Buffer::default();
        assert!(buf.is_empty());
        assert_eq!(buf.height(), 0);
        assert!(!buf.is_dirty());
        assert!(!buf.is_file_loaded());
    }

    // --- 插入 ---

    #[test]
    fn insert_char_creates_line() {
        let mut buf = Buffer::default();
        buf.insert_char('h', loc(0, 0));
        assert_eq!(buf.height(), 1);
        assert!(buf.is_dirty());
        assert_eq!(buf.get_line_content(0), Some("h".to_string()));
    }

    #[test]
    fn insert_multiple_chars() {
        let mut buf = Buffer::default();
        buf.insert_char('h', loc(0, 0));
        buf.insert_char('e', loc(0, 1));
        buf.insert_char('l', loc(0, 2));
        buf.insert_char('l', loc(0, 3));
        buf.insert_char('o', loc(0, 4));
        assert_eq!(buf.get_line_content(0), Some("hello".to_string()));
    }

    // --- 删除 ---

    #[test]
    fn delete_char() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        buf.delete(loc(0, 1));
        assert_eq!(buf.get_line_content(0), Some("hllo".to_string()));
    }

    // --- 换行 ---

    #[test]
    fn insert_newline() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        buf.insert_newline(loc(0, 5));
        assert_eq!(buf.height(), 2);
        assert_eq!(buf.get_line_content(0), Some("hello".to_string()));
        assert_eq!(buf.get_line_content(1), Some(" world".to_string()));
    }

    // --- 删除行 ---

    #[test]
    fn delete_line() {
        let mut buf = Buffer::default();
        buf.set_lines(&["line1".into(), "line2".into(), "line3".into()]);
        let deleted = buf.delete_line(1);
        assert_eq!(deleted, Some("line2".to_string()));
        assert_eq!(buf.height(), 2);
        assert_eq!(buf.get_line_content(0), Some("line1".to_string()));
        assert_eq!(buf.get_line_content(1), Some("line3".to_string()));
    }

    #[test]
    fn delete_only_line() {
        let mut buf = Buffer::default();
        buf.set_lines(&["only".into()]);
        let deleted = buf.delete_line(0);
        assert_eq!(deleted, Some("only".to_string()));
        assert_eq!(buf.height(), 1);
        assert_eq!(buf.get_line_content(0), Some("".to_string()));
    }

    // --- 合并行 ---

    #[test]
    fn join_next_line() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into(), "world".into()]);
        buf.join_next_line(0);
        assert_eq!(buf.height(), 1);
        assert_eq!(buf.get_line_content(0), Some("helloworld".to_string()));
    }

    // --- set_lines / get_all_lines ---

    #[test]
    fn set_lines_and_get_all() {
        let mut buf = Buffer::default();
        buf.set_lines(&["aaa".into(), "bbb".into(), "ccc".into()]);
        assert_eq!(buf.height(), 3);
        assert_eq!(buf.get_all_lines(), vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn set_line_content() {
        let mut buf = Buffer::default();
        buf.set_lines(&["old".into()]);
        buf.set_line_content(0, "new");
        assert_eq!(buf.get_line_content(0), Some("new".to_string()));
    }

    // --- get_line_content 越界 ---

    #[test]
    fn get_line_content_out_of_bounds() {
        let buf = Buffer::default();
        assert_eq!(buf.get_line_content(0), None);
    }

    // --- grapheme_count ---

    #[test]
    fn grapheme_count_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        assert_eq!(buf.grapheme_count(0), 5);
    }

    #[test]
    fn grapheme_count_out_of_bounds() {
        let buf = Buffer::default();
        assert_eq!(buf.grapheme_count(0), 0);
    }

    // --- last_line_idx ---

    #[test]
    fn last_line_idx_empty() {
        let buf = Buffer::default();
        assert_eq!(buf.last_line_idx(), 0);
    }

    #[test]
    fn last_line_idx_non_empty() {
        let mut buf = Buffer::default();
        buf.set_lines(&["a".into(), "b".into(), "c".into()]);
        assert_eq!(buf.last_line_idx(), 2);
    }

    // --- 搜索 ---

    #[test]
    fn search_forward_found() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into(), "world".into()]);
        let result = buf.search_forward("world", loc(0, 0));
        assert_eq!(result, Some(loc(1, 0)));
    }

    #[test]
    fn search_forward_same_line() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        let result = buf.search_forward("world", loc(0, 0));
        assert_eq!(result, Some(loc(0, 6)));
    }

    #[test]
    fn search_forward_wrap_around() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into(), "world".into()]);
        let result = buf.search_forward("hello", loc(1, 0));
        assert_eq!(result, Some(loc(0, 0)));
    }

    #[test]
    fn search_forward_not_found() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        let result = buf.search_forward("xyz", loc(0, 0));
        assert_eq!(result, None);
    }

    #[test]
    fn search_backward_found() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into(), "world".into()]);
        let result = buf.search_backward("hello", loc(1, 0));
        assert_eq!(result, Some(loc(0, 0)));
    }

    #[test]
    fn search_backward_wrap_around() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into(), "world".into()]);
        let result = buf.search_backward("world", loc(0, 0));
        assert_eq!(result, Some(loc(1, 0)));
    }

    // --- 单词导航 ---

    #[test]
    fn next_word_location_start_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world foo".into()]);
        let result = buf.next_word_location_start(&loc(0, 0), WordSeparatorType::Symbol);
        assert_eq!(result, loc(0, 6));
    }

    #[test]
    fn current_or_prev_word_location_start_at_word() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        // 在 word 中间，应该找到当前或前一个 word 的起始
        let result = buf.current_or_prev_word_location_start(&loc(0, 2), WordSeparatorType::Symbol);
        assert_eq!(result.line_idx, 0);
        // 结果应该是某个 word 的起始位置
        assert!(result.grapheme_idx <= 2);
    }

    // --- 括号匹配 ---

    #[test]
    fn find_matching_bracket_forward() {
        let mut buf = Buffer::default();
        buf.set_lines(&["(hello)".into()]);
        let result = buf.find_matching_bracket(loc(0, 0), '(', ')', true);
        assert_eq!(result, Some(loc(0, 6)));
    }

    #[test]
    fn find_matching_bracket_backward() {
        let mut buf = Buffer::default();
        buf.set_lines(&["(hello)".into()]);
        let result = buf.find_matching_bracket(loc(0, 6), '(', ')', false);
        assert_eq!(result, Some(loc(0, 0)));
    }

    #[test]
    fn find_matching_bracket_nested() {
        let mut buf = Buffer::default();
        buf.set_lines(&["(a(b)c)".into()]);
        let result = buf.find_matching_bracket(loc(0, 0), '(', ')', true);
        assert_eq!(result, Some(loc(0, 6)));
    }

    #[test]
    fn find_matching_bracket_not_found() {
        let mut buf = Buffer::default();
        buf.set_lines(&["(hello".into()]);
        let result = buf.find_matching_bracket(loc(0, 0), '(', ')', true);
        assert_eq!(result, None);
    }

    // --- inner_word_range ---

    #[test]
    fn inner_word_range_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        let result = buf.inner_word_range(&loc(0, 2));
        assert_eq!(result, Some((loc(0, 0), loc(0, 5))));
    }

    #[test]
    fn inner_word_range_on_space() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        let result = buf.inner_word_range(&loc(0, 5));
        assert_eq!(result, None);
    }

    // --- find_char_on_line ---

    #[test]
    fn find_char_on_line_forward() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        let result = buf.find_char_on_line(0, 0, 'l', true);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn find_char_on_line_backward() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        let result = buf.find_char_on_line(0, 4, 'l', false);
        assert_eq!(result, Some(3));
    }

    // --- find_next_word_start_on_line ---

    #[test]
    fn find_next_word_start_on_line_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world foo".into()]);
        let result = buf.find_next_word_start_on_line(0, 0);
        assert_eq!(result, Some(6));
    }

    // --- delete_word_forward / delete_word_backward ---

    #[test]
    fn delete_word_forward_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        buf.delete_word_forward(loc(0, 0));
        assert_eq!(buf.get_line_content(0), Some(" world".to_string()));
    }

    #[test]
    fn delete_word_backward_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello world".into()]);
        buf.delete_word_backward(loc(0, 11));
        assert_eq!(buf.get_line_content(0), Some("hello ".to_string()));
    }

    // --- save / load ---

    #[test]
    fn save_as_and_load() {
        let tmp = std::env::temp_dir().join("editor_test_buffer_save.txt");
        let path = tmp.to_str().unwrap().to_string();

        let mut buf = Buffer::default();
        buf.set_lines(&["line1".into(), "line2".into()]);
        buf.save_as(&path).unwrap();

        let loaded = Buffer::load(&path).unwrap();
        assert_eq!(loaded.height(), 2);
        assert_eq!(loaded.get_line_content(0), Some("line1".to_string()));
        assert_eq!(loaded.get_line_content(1), Some("line2".to_string()));

        let _ = std::fs::remove_file(&tmp);
    }

    // --- grapheme_to_byte ---

    #[test]
    fn grapheme_to_byte_ascii() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        assert_eq!(buf.grapheme_to_byte(0, 0), 0);
        assert_eq!(buf.grapheme_to_byte(0, 3), 3);
        assert_eq!(buf.grapheme_to_byte(0, 5), 5);
    }

    #[test]
    fn grapheme_to_byte_cjk() {
        let mut buf = Buffer::default();
        buf.set_lines(&["你好".into()]);
        assert_eq!(buf.grapheme_to_byte(0, 0), 0);
        assert_eq!(buf.grapheme_to_byte(0, 1), 3);
        assert_eq!(buf.grapheme_to_byte(0, 2), 6);
    }

    // --- column_to_byte ---

    #[test]
    fn column_to_byte_basic() {
        let mut buf = Buffer::default();
        buf.set_lines(&["hello".into()]);
        assert_eq!(buf.column_to_byte(0, 0), 0);
        assert_eq!(buf.column_to_byte(0, 3), 3);
    }
}
