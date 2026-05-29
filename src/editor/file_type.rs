use std::fmt::{Display, Formatter};

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileType {
    #[default]
    Text,
    Rust
}

impl Display for FileType {
    
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "Text"),
            Self::Rust => write!(f, "Rust"),
        }
    }
}