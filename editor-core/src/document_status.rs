use crate::file_type::FileType;
use crate::prelude::*;

#[derive(Default, Eq, PartialEq, Debug)]
pub struct DocumentStatus {
    pub total_lines: usize,
    pub current_line_idx: LineIdx,
    pub is_modified: bool,
    pub filename: String,
    pub file_type: FileType,
    pub selection_count: Option<usize>,
}

impl DocumentStatus {
    pub fn modified_indicator_to_string(&self) -> String {
        if self.is_modified {
            "(modified)".to_string()
        } else {
            String::new()
        }
    }

    pub fn line_count_to_string(&self) -> String {
        let lines = if self.total_lines > 0 {
            self.total_lines
        } else {
            1
        };
        format!("{} lines", lines)
    }

    pub fn position_indicator_to_string(&self) -> String {
        let lines = if self.total_lines > 0 {
            self.total_lines
        } else {
            1
        };
        format!("{}/{}", self.current_line_idx.saturating_add(1), lines)
    }

    pub fn selection_indicator_to_string(&self) -> String {
        self.selection_count
            .map_or(String::new(), |n| format!(" [{}]", n))
    }

    pub fn file_type_to_string(&self) -> String {
        self.file_type.to_string()
    }
}
