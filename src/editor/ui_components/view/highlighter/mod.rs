use crate::editor::annotation::Annotation;
use crate::editor::file_type::FileType;
use crate::editor::line::Line;
use crate::editor::ui_components::view::highlighter::rust_syntax_highlighter::RustSyntaxHighlighter;
use crate::editor::ui_components::view::highlighter::search_result_highlighter::SearchResultHighlighter;
use crate::editor::ui_components::view::highlighter::syntax_highlighter::SyntaxHighlighter;
use crate::prelude::{LineIdx, Location};

mod syntax_highlighter;
mod rust_syntax_highlighter;
mod search_result_highlighter;

fn create_syntax_highlighter(file_type: FileType) -> Option<Box<dyn SyntaxHighlighter>> {
    match file_type {
        FileType::Rust => Some(Box::new(RustSyntaxHighlighter::default())),
        FileType::Text => None
    }
}

#[derive(Default)]
pub struct Highlighter<'a> {
    syntax_highlighter: Option<Box<dyn SyntaxHighlighter>>,
    search_result_highlighter: Option<SearchResultHighlighter<'a>>
}

impl<'a> Highlighter<'a> {

    pub fn new(matched_word: Option<&'a str>, selected_match: Option<Location>, file_type: FileType) -> Self {
        let search_result_highlighter = matched_word.map(|matched_word| SearchResultHighlighter::new(matched_word, selected_match));
        Self {
            syntax_highlighter: create_syntax_highlighter(file_type),
            search_result_highlighter
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