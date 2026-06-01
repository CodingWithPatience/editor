use crate::annotation::Annotation;
use crate::annotation_type::AnnotationType;
use crate::highlighter::syntax_highlighter::SyntaxHighlighter;
use crate::line::Line;
use crate::prelude::{LineIdx, Location};
use std::collections::HashMap;

#[derive(Default)]
pub struct SearchResultHighlighter<'a> {
    matched_word: &'a str,
    selected_match: Option<Location>,
    highlights: HashMap<LineIdx, Vec<Annotation>>,
}

impl<'a> SearchResultHighlighter<'a> {
    pub fn new(matched_word: &'a str, selected_match: Option<Location>) -> Self {
        Self {
            matched_word,
            selected_match,
            highlights: HashMap::new(),
        }
    }

    fn highlight_matched_words(&self, line: &Line, result: &mut Vec<Annotation>) {
        if self.matched_word.is_empty() {
            return;
        }
        line.find_all(self.matched_word, 0..line.len())
            .iter()
            .for_each(|(start, _)| {
                result.push(Annotation {
                    annotation_type: AnnotationType::Match,
                    start: *start,
                    end: start.saturating_add(self.matched_word.len()),
                });
            });
    }

    fn highlight_selected_match(&self, line: &Line, result: &mut Vec<Annotation>) {
        if let Some(selected_match) = self.selected_match {
            if self.matched_word.is_empty() {
                return;
            }
            let start = line.grapheme_idx_to_byte_idx(selected_match.grapheme_idx);
            result.push(Annotation {
                annotation_type: AnnotationType::SelectedMatch,
                start,
                end: start.saturating_add(self.matched_word.len()),
            })
        }
    }
}

impl<'a> SyntaxHighlighter for SearchResultHighlighter<'a> {
    fn highlight(&mut self, line_idx: LineIdx, line: &Line) {
        let mut result = Vec::new();
        self.highlight_matched_words(line, &mut result);
        if let Some(selected_match) = self.selected_match {
            if selected_match.line_idx == line_idx && !result.is_empty() {
                self.highlight_selected_match(line, &mut result);
            }
        }
        self.highlights.insert(line_idx, result);
    }

    fn get_annotations(&self, line_idx: LineIdx) -> Option<&Vec<Annotation>> {
        self.highlights.get(&line_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlighter::syntax_highlighter::SyntaxHighlighter;

    #[test]
    fn new_empty_query() {
        let mut hl = SearchResultHighlighter::new("", None);
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0).unwrap();
        assert!(annotations.is_empty());
    }

    #[test]
    fn highlight_single_match() {
        let mut hl = SearchResultHighlighter::new("world", None);
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0).unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].annotation_type, AnnotationType::Match);
    }

    #[test]
    fn highlight_multiple_matches() {
        let mut hl = SearchResultHighlighter::new("ab", None);
        let line = Line::from("ab ab ab");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0).unwrap();
        assert_eq!(annotations.len(), 3);
    }

    #[test]
    fn highlight_selected_match() {
        let selected = Location {
            line_idx: 0,
            grapheme_idx: 6,
        };
        let mut hl = SearchResultHighlighter::new("world", Some(selected));
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0).unwrap();
        assert_eq!(annotations.len(), 2);
        assert!(annotations.iter().any(|a| a.annotation_type == AnnotationType::Match));
        assert!(annotations.iter().any(|a| a.annotation_type == AnnotationType::SelectedMatch));
    }

    #[test]
    fn highlight_selected_on_different_line() {
        let selected = Location {
            line_idx: 1,
            grapheme_idx: 0,
        };
        let mut hl = SearchResultHighlighter::new("world", Some(selected));
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0).unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].annotation_type, AnnotationType::Match);
    }

    #[test]
    fn highlight_no_match() {
        let mut hl = SearchResultHighlighter::new("xyz", None);
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0).unwrap();
        assert!(annotations.is_empty());
    }

    #[test]
    fn get_annotations_before_highlight() {
        let hl = SearchResultHighlighter::new("test", None);
        assert!(hl.get_annotations(0).is_none());
    }

    #[test]
    fn highlight_multiple_lines() {
        let mut hl = SearchResultHighlighter::new("hello", None);
        let line0 = Line::from("hello world");
        let line1 = Line::from("say hello");
        hl.highlight(0, &line0);
        hl.highlight(1, &line1);
        assert_eq!(hl.get_annotations(0).unwrap().len(), 1);
        assert_eq!(hl.get_annotations(1).unwrap().len(), 1);
    }
}
