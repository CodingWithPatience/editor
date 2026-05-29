#[derive(Copy, Clone, Debug)]
pub enum GraphemeWidth {
    Half,
    Full,
}

impl From<GraphemeWidth> for usize {
    fn from(value: GraphemeWidth) -> Self {
        match value {
            GraphemeWidth::Half => 1,
            GraphemeWidth::Full => 2,
        }
    }
}
