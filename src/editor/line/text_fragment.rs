use crate::editor::line::ByteIdx;
use crate::editor::line::grapheme_width::GraphemeWidth;

#[derive(Clone, Debug)]
pub struct TextFragment {
    pub grapheme: String,
    pub rendered_width: GraphemeWidth,
    pub replacement: Option<char>,
    pub start: ByteIdx,
}