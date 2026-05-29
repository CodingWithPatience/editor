use crate::annotated_string::AnnotatedString;
use crate::annotation::Annotation;
use crate::prelude::{ByteIdx, ColIdx, GraphemeIdx, GraphemeType, WordSeparatorType, WordType};
use grapheme_width::GraphemeWidth;
use std::cmp::min;
use std::fmt;
use std::fmt::Formatter;
use std::ops::{Deref, Range};
use text_fragment::TextFragment;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

mod grapheme_width;
mod text_fragment;

#[derive(Default, Clone)]
pub struct Line {
    fragments: Vec<TextFragment>,
    string: String,
}

impl Line {
    pub fn from(line_str: &str) -> Self {
        debug_assert!(line_str.is_empty() || line_str.lines().count() == 1);
        let fragments = Self::str_to_fragments(line_str);
        Self {
            fragments,
            string: String::from(line_str),
        }
    }

    pub fn current_or_next_word_end_grapheme_idx(
        &self,
        current_grapheme_idx: GraphemeIdx,
        separator_type: WordSeparatorType,
        start_at_prev: bool,
        reverse: bool,
    ) -> Option<GraphemeIdx> {
        if self.fragments.len() == 0 {
            return None;
        }
        let mut grapheme_record = (0, 0, false);
        grapheme_record.2 = start_at_prev;
        let mut word_type = WordType::None;

        let mut f = |i: usize, s: &str, reverse: bool| {
            let grapheme_type = GraphemeType::from(s);
            if word_type == WordType::None {
                word_type = WordType::from(grapheme_type, separator_type);
            }
            let target_idx = if reverse {
                i.saturating_add(1)
            } else {
                i.saturating_sub(1)
            };
            match grapheme_type {
                GraphemeType::Keyword => {
                    if word_type == WordType::PureSymbol {
                        if grapheme_record.1 > 1 || grapheme_record.2 && grapheme_record.1 == 1 {
                            return Some(target_idx);
                        }
                        word_type = if separator_type == WordSeparatorType::Symbol {
                            if grapheme_record.1 == 1 {
                                grapheme_record.2 = true;
                            }
                            WordType::PureKeyword
                        } else {
                            WordType::Mix
                        }
                    }
                    grapheme_record.0 += 1;
                }
                GraphemeType::Symbol => {
                    if word_type == WordType::PureKeyword {
                        if grapheme_record.0 > 1 || (grapheme_record.2 && grapheme_record.0 == 1) {
                            return Some(target_idx);
                        } else if grapheme_record.0 == 1 {
                            grapheme_record.2 = true;
                            word_type = WordType::PureSymbol;
                        }
                    }
                    grapheme_record.1 += 1;
                }
                GraphemeType::Whitespace => {
                    if word_type != WordType::None {
                        if (grapheme_record.0 + grapheme_record.1) > 1
                            || (grapheme_record.2 && (grapheme_record.0 + grapheme_record.1) == 1)
                        {
                            return Some(target_idx);
                        }
                    }
                    grapheme_record.2 = true;
                    word_type = WordType::None;
                }
            }
            let end_idx = if reverse {
                0
            } else {
                self.fragments.len().saturating_sub(1)
            };
            if i == end_idx {
                if grapheme_type != GraphemeType::Whitespace && grapheme_record.2 {
                    return Some(i);
                }
                // Cursor was inside a word that extends to the line boundary.
                // Only trigger when we traversed >1 fragment (mid-word), not
                // when cursor is at the last char (single fragment → cross lines).
                let frag_count = grapheme_record.0 + grapheme_record.1;
                if word_type != WordType::None && frag_count > 1 {
                    return Some(i);
                }
            }
            None
        };

        let enumerate = self.fragments.iter().enumerate();

        if reverse {
            for (i, t) in enumerate.rev().skip(
                self.fragments
                    .len()
                    .saturating_sub(1)
                    .saturating_sub(current_grapheme_idx),
            ) {
                let r = f(i, t.grapheme.as_str(), true);
                if r.is_some() {
                    return r;
                }
            }
        } else {
            for (i, t) in enumerate.skip(current_grapheme_idx) {
                let r = f(i, t.grapheme.as_str(), false);
                if r.is_some() {
                    return r;
                }
            }
        }
        None
    }

    pub fn next_word_start_grapheme_idx(
        &self,
        current_grapheme_idx: GraphemeIdx,
        separator_type: WordSeparatorType,
        start_at_prev: bool,
    ) -> Option<GraphemeIdx> {
        if self.fragments.len() == 0 {
            return None;
        }
        let mut grapheme_record = (false, false, false);
        grapheme_record.2 = start_at_prev;
        for (i, t) in self.fragments.iter().enumerate().skip(current_grapheme_idx) {
            let grapheme_type = GraphemeType::from(t.grapheme.as_str());
            match grapheme_type {
                GraphemeType::Keyword => {
                    if grapheme_record.2
                        || (matches!(separator_type, WordSeparatorType::Symbol)
                            && grapheme_record.1)
                    {
                        return Some(i);
                    }
                    grapheme_record.0 = true;
                }
                GraphemeType::Symbol => {
                    if grapheme_record.2 {
                        return Some(i);
                    }
                    if matches!(separator_type, WordSeparatorType::Symbol) {
                        if grapheme_record.0 {
                            return Some(i);
                        }
                        grapheme_record.1 = true;
                    } else {
                        grapheme_record.0 = true;
                    }
                }
                GraphemeType::Whitespace => grapheme_record.2 = true,
            }
        }
        None
    }

    /// 在当前行中从 from 位置开始向后查找字符 ch，返回匹配的 grapheme 下标。
    pub fn find_char_forward(&self, from: GraphemeIdx, ch: char) -> Option<GraphemeIdx> {
        // Start searching AFTER the current position
        let start = from.saturating_add(1).min(self.grapheme_count());
        for (i, fragment) in self.fragments.iter().enumerate().skip(start) {
            if fragment.grapheme == ch.to_string() {
                return Some(i);
            }
        }
        None
    }

    /// 在当前行中从 from 位置开始向前查找字符 ch，返回匹配的 grapheme 下标。
    pub fn find_char_backward(&self, from: GraphemeIdx, ch: char) -> Option<GraphemeIdx> {
        let end = from.min(self.grapheme_count());
        for (i, fragment) in self.fragments.iter().enumerate().take(end).rev() {
            if fragment.grapheme == ch.to_string() {
                return Some(i);
            }
        }
        None
    }

    pub fn first_visible_grapheme_idx(&self) -> GraphemeIdx {
        if self.fragments.len() == 0 {
            return 0;
        }
        for (i, fragment) in self.fragments.iter().enumerate() {
            let grapheme_type = GraphemeType::from(fragment.grapheme.as_str());
            if grapheme_type == GraphemeType::Whitespace {
                continue;
            }
            return i;
        }
        self.fragments.len().saturating_sub(1)
    }

    pub fn get_annotated_visible_substr(
        &self,
        range: Range<ColIdx>,
        annotations: Option<&Vec<Annotation>>,
    ) -> AnnotatedString {
        if range.start >= range.end {
            return AnnotatedString::default();
        }
        let mut result = AnnotatedString::from(&self.string);
        if let Some(annotations) = annotations {
            for annotation in annotations {
                result.add_annotation(annotation.annotation_type, annotation.start, annotation.end);
            }
        }

        let mut fragment_start = self.width();
        for fragment in self.fragments.iter().rev() {
            let fragment_end = fragment_start;
            fragment_start = fragment_start.saturating_sub(fragment.rendered_width.into());
            if fragment_start > range.end {
                continue;
            }
            if fragment_start < range.end && fragment_end > range.end {
                result.replace(fragment.start, self.string.len(), "...");
                continue;
            } else if fragment_start == range.end {
                result.truncate_right_from(fragment.start);
                continue;
            }
            if fragment_end <= range.start {
                result.truncate_left_until(fragment.start.saturating_add(fragment.grapheme.len()));
                break;
            } else if fragment_start < range.start && fragment_end > range.start {
                result.replace(
                    0,
                    fragment.start.saturating_add(fragment.grapheme.len()),
                    "...",
                );
                break;
            }
            if fragment_start >= range.start && fragment_end <= range.end {
                if let Some(replacement) = fragment.replacement {
                    let start = fragment.start;
                    let end = start.saturating_add(fragment.grapheme.len());
                    result.replace(start, end, &replacement.to_string());
                }
            }
        }
        result
    }

    pub fn insert_char(&mut self, character: char, at: GraphemeIdx) {
        debug_assert!(at.saturating_sub(1) <= self.grapheme_count());
        if let Some(fragment) = self.fragments.get(at) {
            self.string.insert(fragment.start, character);
        } else {
            self.string.push(character);
        }
        self.rebuild_fragments();
    }

    pub fn append_char(&mut self, character: char) {
        self.insert_char(character, self.grapheme_count());
    }

    pub fn append_char_str(&mut self, s: &str) {
        self.string.push_str(s);
        self.rebuild_fragments();
    }

    pub fn delete(&mut self, at: GraphemeIdx) {
        debug_assert!(at <= self.grapheme_count());
        if let Some(fragment) = self.fragments.get(at) {
            let start = fragment.start;
            let end = fragment.start.saturating_add(fragment.grapheme.len());
            self.string.drain(start..end);
            self.rebuild_fragments();
        }
    }

    pub fn delete_word_forward_from(&mut self, at: GraphemeIdx) -> GraphemeIdx {
        debug_assert!(at <= self.grapheme_count());
        if at >= self.grapheme_count() {
            return 0;
        }
        let end = self
            .current_or_next_word_end_grapheme_idx(at, WordSeparatorType::Symbol, false, false)
            .map(|g| g.saturating_add(1))
            .unwrap_or_else(|| self.grapheme_count());
        let count = end.saturating_sub(at);
        for _ in 0..count {
            self.delete(at);
        }
        count
    }

    pub fn delete_word_backward_from(&mut self, at: GraphemeIdx) -> GraphemeIdx {
        debug_assert!(at <= self.grapheme_count());
        if at == 0 {
            return 0;
        }
        let start_idx = at
            .saturating_sub(1)
            .min(self.grapheme_count().saturating_sub(1));
        let start = self
            .current_or_next_word_end_grapheme_idx(start_idx, WordSeparatorType::Symbol, true, true)
            .unwrap_or(0);
        let count = at.saturating_sub(start);
        for _ in 0..count {
            self.delete(start);
        }
        start
    }

    pub fn split(&mut self, at: GraphemeIdx) -> Self {
        if let Some(fragment) = self.fragments.get(at) {
            let remainder = self.string.split_off(fragment.start);
            self.rebuild_fragments();
            Self::from(&remainder)
        } else {
            Self::default()
        }
    }

    pub fn append(&mut self, other: &Self) {
        self.string.push_str(&other.string);
        self.rebuild_fragments();
    }

    pub fn get_visible_graphemes(&self, range: Range<ColIdx>) -> String {
        self.get_annotated_visible_substr(range, None).to_string()
    }

    pub fn grapheme_count(&self) -> GraphemeIdx {
        self.fragments.len()
    }

    pub fn width_until(&self, grapheme_idx: GraphemeIdx) -> ColIdx {
        self.fragments
            .iter()
            .take(grapheme_idx)
            .map(|fragment| match fragment.rendered_width {
                GraphemeWidth::Half => 1,
                GraphemeWidth::Full => 2,
            })
            .sum()
    }

    pub fn width(&self) -> ColIdx {
        self.width_until(self.grapheme_count())
    }

    pub fn search_forward(
        &self,
        query: &str,
        from_grapheme_idx: GraphemeIdx,
    ) -> Option<GraphemeIdx> {
        debug_assert!(from_grapheme_idx <= self.grapheme_count());
        if from_grapheme_idx == self.grapheme_count() {
            return None;
        }
        let start = self.grapheme_idx_to_byte_idx(from_grapheme_idx);
        self.find_all(query, start..self.string.len())
            .first()
            .map(|(_, grapheme_idx)| *grapheme_idx)
    }

    pub fn search_backward(
        &self,
        query: &str,
        from_grapheme_idx: GraphemeIdx,
    ) -> Option<GraphemeIdx> {
        debug_assert!(from_grapheme_idx <= self.grapheme_count());
        if from_grapheme_idx == 0 {
            return None;
        }
        let end = if from_grapheme_idx == self.grapheme_count() {
            self.string.len()
        } else {
            self.grapheme_idx_to_byte_idx(from_grapheme_idx)
        };
        self.find_all(query, 0..end)
            .first()
            .map(|(_, grapheme_idx)| *grapheme_idx)
    }

    fn get_replacement_character(for_str: &str) -> Option<char> {
        let width = for_str.width();
        match for_str {
            " " => None,
            "\t" => Some(' '),
            _ if width > 0 && for_str.trim().is_empty() => Some('_'),
            _ if width == 0 => {
                let mut chars = for_str.chars();
                if let Some(ch) = chars.next() {
                    if ch.is_control() && chars.next().is_none() {
                        return Some('▯');
                    }
                }
                Some('.')
            }
            _ => None,
        }
    }

    fn rebuild_fragments(&mut self) {
        self.fragments = Self::str_to_fragments(&self.string);
    }

    fn byte_idx_to_grapheme_idx(&self, byte_idx: ByteIdx) -> Option<GraphemeIdx> {
        if byte_idx > self.string.len() {
            return None;
        }
        self.fragments
            .iter()
            .position(|fragment| fragment.start >= byte_idx)
    }

    /// Convert a column position (display width) to a byte offset in the raw string.
    pub fn column_to_byte(&self, col: ColIdx) -> ByteIdx {
        let mut current_col: usize = 0;
        for fragment in &self.fragments {
            let fragment_width: usize = fragment.rendered_width.into();
            if current_col.saturating_add(fragment_width) > col {
                return fragment.start;
            }
            current_col = current_col.saturating_add(fragment_width);
        }
        self.string.len()
    }

    pub fn grapheme_idx_to_byte_idx(&self, grapheme_idx: GraphemeIdx) -> ByteIdx {
        debug_assert!(grapheme_idx <= self.grapheme_count());
        if grapheme_idx == 0 || self.grapheme_count() == 0 {
            return 0;
        }
        if grapheme_idx == self.grapheme_count() {
            return self.string.len();
        }
        self.fragments.get(grapheme_idx).map_or_else(
            || {
                #[cfg(debug_assertions)]
                {
                    panic!("Fragment not found for grapheme index {grapheme_idx:?}");
                }
                #[cfg(not(debug_assertions))]
                {
                    0
                }
            },
            |fragment| fragment.start,
        )
    }

    fn str_to_fragments(line_str: &str) -> Vec<TextFragment> {
        line_str
            .grapheme_indices(true)
            .map(|(byte_idx, grapheme)| {
                let (replacement, rendered_width) = Self::get_replacement_character(grapheme)
                    .map_or_else(
                        || {
                            let unicode_width = grapheme.width();
                            let rendered_width = match unicode_width {
                                0 | 1 => GraphemeWidth::Half,
                                _ => GraphemeWidth::Full,
                            };
                            (None, rendered_width)
                        },
                        |replacement| (Some(replacement), GraphemeWidth::Half),
                    );

                TextFragment {
                    grapheme: grapheme.to_string(),
                    rendered_width,
                    replacement,
                    start: byte_idx,
                }
            })
            .collect()
    }

    pub fn find_all(&self, query: &str, range: Range<ByteIdx>) -> Vec<(ByteIdx, GraphemeIdx)> {
        let start = range.start;
        let end = min(range.end, self.string.len());
        debug_assert!(start <= end);
        self.string.get(start..end).map_or_else(Vec::new, |substr| {
            let potential_matches: Vec<ByteIdx> = substr
                .match_indices(query)
                .map(|(relative_start_idx, _)| relative_start_idx.saturating_add(start))
                .collect();
            self.match_grapheme_clusters(&potential_matches, query)
        })
    }

    fn match_grapheme_clusters(
        &self,
        matches: &[ByteIdx],
        query: &str,
    ) -> Vec<(ByteIdx, GraphemeIdx)> {
        let grapheme_count = query.graphemes(true).count();
        matches
            .iter()
            .filter_map(|&start| {
                self.byte_idx_to_grapheme_idx(start)
                    .and_then(|grapheme_idx| {
                        self.fragments
                            .get(grapheme_idx..grapheme_idx.saturating_add(grapheme_count))
                            .and_then(|fragments| {
                                let substring = fragments
                                    .iter()
                                    .map(|fragment| fragment.grapheme.as_str())
                                    .collect::<String>();
                                (substring == query).then_some((start, grapheme_idx))
                            })
                    })
            })
            .collect()
    }
}

impl fmt::Display for Line {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "{}", self.string)
    }
}

impl Deref for Line {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}
