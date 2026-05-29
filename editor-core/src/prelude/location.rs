use crate::prelude::{GraphemeIdx, LineIdx};

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct Location {
    pub grapheme_idx: GraphemeIdx,
    pub line_idx: LineIdx,
}
