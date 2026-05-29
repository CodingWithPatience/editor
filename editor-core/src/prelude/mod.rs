pub type GraphemeIdx = usize;
pub type LineIdx = usize;
pub type ByteIdx = usize;
pub type ColIdx = usize;
pub type RowIdx = usize;

/// 行号区域的字符宽度
pub const GUTTER_WIDTH: usize = 6;

mod grapheme_type;
mod location;
mod position;
mod size;

pub use grapheme_type::GraphemeType;
pub use grapheme_type::WordSeparatorType;
pub use grapheme_type::WordType;
pub use location::Location;
pub use position::Position;
pub use size::Size;
