use crate::prelude::Size;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum System {
    Save,
    Resize(Size),
    Quit,
    Dismiss,
    Search,
    SearchBackward,
    SearchWordForward,
    SearchWordBackward,
    CommandMode,
}
