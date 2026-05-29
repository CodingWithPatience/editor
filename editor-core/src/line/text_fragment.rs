use crate::line::grapheme_width::GraphemeWidth;
use crate::prelude::ByteIdx;

#[derive(Clone, Debug)]
pub struct TextFragment {
    pub grapheme: String,
    pub rendered_width: GraphemeWidth,
    pub replacement: Option<char>,
    pub start: ByteIdx,
}
