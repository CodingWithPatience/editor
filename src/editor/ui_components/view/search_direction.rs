#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward
}