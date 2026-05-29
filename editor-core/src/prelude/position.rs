use crate::prelude::{ColIdx, RowIdx};

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct Position {
    pub col: ColIdx,
    pub row: RowIdx,
}

impl Position {
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self {
            col: self.col.saturating_sub(other.col),
            row: self.row.saturating_sub(other.row),
        }
    }
}
