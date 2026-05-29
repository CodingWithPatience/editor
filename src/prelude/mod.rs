pub type GraphemeIdx = usize;
pub type LineIdx = usize;
pub type ByteIdx = usize;
pub type ColIdx = usize;
pub type RowIdx = usize;

mod position;
mod location;
mod size;
mod grapheme_type;

pub use position::Position;
pub use size::Size;
pub use location::Location;
pub use grapheme_type::GraphemeType;
pub use grapheme_type::WordType;
pub use grapheme_type::WordSeparatorType;