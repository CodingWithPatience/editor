#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AnnotationType {
    Match,
    SelectedMatch,
    Number,
    Keyword,
    Type,
    KnownValue,
    Char,
    LifetimeSpecifier,
    Comment,
    String,
    LineNumber,
    LineNumberSection,
    Selection,
}
