use crate::annotation::Annotation;
use crate::file_type::FileType;
use crate::highlighter::rust_syntax_highlighter::RustSyntaxHighlighter;
use crate::highlighter::search_result_highlighter::SearchResultHighlighter;
use crate::highlighter::syntax_highlighter::SyntaxHighlighter;
use crate::line::Line;
use crate::prelude::{LineIdx, Location};

mod rust_syntax_highlighter;
mod search_result_highlighter;
mod syntax_highlighter;

fn create_syntax_highlighter(file_type: FileType) -> Option<Box<dyn SyntaxHighlighter>> {
    match file_type {
        FileType::Rust => Some(Box::new(RustSyntaxHighlighter::default())),
        FileType::Text => None,
    }
}

#[derive(Default)]
pub struct Highlighter<'a> {
    syntax_highlighter: Option<Box<dyn SyntaxHighlighter>>,
    search_result_highlighter: Option<SearchResultHighlighter<'a>>,
}

impl<'a> Highlighter<'a> {
    pub fn new(
        matched_word: Option<&'a str>,
        selected_match: Option<Location>,
        file_type: FileType,
    ) -> Self {
        let search_result_highlighter = matched_word
            .map(|matched_word| SearchResultHighlighter::new(matched_word, selected_match));
        Self {
            syntax_highlighter: create_syntax_highlighter(file_type),
            search_result_highlighter,
        }
    }

    pub fn get_annotations(&self, line_idx: LineIdx) -> Vec<Annotation> {
        let mut result = Vec::new();
        if let Some(syntax_highlighter) = &self.syntax_highlighter {
            if let Some(annotations) = syntax_highlighter.get_annotations(line_idx) {
                result.extend(annotations.iter().copied());
            }
        }
        if let Some(search_result_highlighter) = &self.search_result_highlighter {
            if let Some(annotations) = search_result_highlighter.get_annotations(line_idx) {
                result.extend(annotations.iter().copied());
            }
        }
        result
    }

    pub fn highlight(&mut self, line_idx: LineIdx, line: &Line) {
        if let Some(syntax_highlighter) = &mut self.syntax_highlighter {
            syntax_highlighter.highlight(line_idx, line);
        }
        if let Some(search_result_highlighter) = &mut self.search_result_highlighter {
            search_result_highlighter.highlight(line_idx, line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation_type::AnnotationType;

    #[test]
    fn new_text_file_no_syntax() {
        let hl = Highlighter::new(None, None, FileType::Text);
        assert!(hl.syntax_highlighter.is_none());
        assert!(hl.search_result_highlighter.is_none());
    }

    #[test]
    fn new_rust_file_has_syntax() {
        let hl = Highlighter::new(None, None, FileType::Rust);
        assert!(hl.syntax_highlighter.is_some());
    }

    #[test]
    fn new_with_search_query() {
        let hl = Highlighter::new(Some("hello"), None, FileType::Text);
        assert!(hl.search_result_highlighter.is_some());
    }

    #[test]
    fn highlight_text_file_no_search() {
        let mut hl = Highlighter::new(None, None, FileType::Text);
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0);
        assert!(annotations.is_empty());
    }

    #[test]
    fn highlight_with_search_match() {
        let mut hl = Highlighter::new(Some("world"), None, FileType::Text);
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0);
        assert!(!annotations.is_empty());
        assert!(annotations.iter().any(|a| a.annotation_type == AnnotationType::Match));
    }

    #[test]
    fn highlight_with_selected_match() {
        let selected = Location {
            line_idx: 0,
            grapheme_idx: 6,
        };
        let mut hl = Highlighter::new(Some("world"), Some(selected), FileType::Text);
        let line = Line::from("hello world");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0);
        assert!(annotations.iter().any(|a| a.annotation_type == AnnotationType::SelectedMatch));
    }

    #[test]
    fn highlight_rust_file() {
        let mut hl = Highlighter::new(None, None, FileType::Rust);
        let line = Line::from("fn main() {}");
        hl.highlight(0, &line);
        let annotations = hl.get_annotations(0);
        // Rust 语法高亮应产生 Keyword 注解
        assert!(annotations.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
    }

    #[test]
    fn get_annotations_empty_before_highlight() {
        let hl = Highlighter::new(None, None, FileType::Text);
        let annotations = hl.get_annotations(0);
        assert!(annotations.is_empty());
    }
}
