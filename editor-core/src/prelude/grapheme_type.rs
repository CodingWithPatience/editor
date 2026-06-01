#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WordType {
    Mix,
    PureKeyword,
    PureSymbol,
    None,
}

impl WordType {
    pub fn from(first_char_type: GraphemeType, separator_type: WordSeparatorType) -> Self {
        match first_char_type {
            GraphemeType::Keyword => {
                if matches!(separator_type, WordSeparatorType::Symbol) {
                    WordType::PureKeyword
                } else {
                    WordType::Mix
                }
            }
            GraphemeType::Symbol => {
                if matches!(separator_type, WordSeparatorType::Whitespace) {
                    WordType::Mix
                } else {
                    WordType::PureSymbol
                }
            }
            GraphemeType::Whitespace => WordType::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphemeType {
    Keyword,
    Symbol,
    Whitespace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WordSeparatorType {
    Symbol,
    Whitespace,
}

impl GraphemeType {
    pub fn from(s: &str) -> Self {
        if GraphemeType::is_invisible(s) {
            GraphemeType::Whitespace
        } else if GraphemeType::is_symbol(s) {
            GraphemeType::Symbol
        } else {
            GraphemeType::Keyword
        }
    }

    fn is_symbol(s: &str) -> bool {
        match s {
            "!" | "'" | "#" | "$" | "%" | "&" | "\"" | "(" | ")" | "*" | "+" | "," | "-" | "."
            | "/" | ":" | ";" | "<" | "=" | ">" | "?" | "@" | "[" | "\\" | "]" | "^" | "`"
            | "{" | "|" | "}" | "~" => true,
            "，" | "。" | "；" | "：" | "！" | "？" | "、" | "「" | "」" | "『" | "』" | "《"
            | "》" | "〈" | "〉" | "【" | "】" | "〔" | "〕" | "（" | "）" | "［" | "］" | "｛"
            | "｝" | "·" | "～" | "￥" | "…" | "—" | "｜" => true,
            _ => GraphemeType::is_invisible(s),
        }
    }

    fn is_invisible(s: &str) -> bool {
        s.is_empty() || s.chars().all(|c| c.is_whitespace() || c.is_control())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- GraphemeType 分类 ---

    #[test]
    fn keyword_letters() {
        assert_eq!(GraphemeType::from("a"), GraphemeType::Keyword);
        assert_eq!(GraphemeType::from("Z"), GraphemeType::Keyword);
        assert_eq!(GraphemeType::from("hello"), GraphemeType::Keyword);
    }

    #[test]
    fn keyword_digits() {
        assert_eq!(GraphemeType::from("0"), GraphemeType::Keyword);
        assert_eq!(GraphemeType::from("9"), GraphemeType::Keyword);
        assert_eq!(GraphemeType::from("123"), GraphemeType::Keyword);
    }

    #[test]
    fn keyword_underscore() {
        assert_eq!(GraphemeType::from("_"), GraphemeType::Keyword);
    }

    #[test]
    fn symbol_ascii() {
        assert_eq!(GraphemeType::from("!"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("+"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("["), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("]"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("{"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("}"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("("), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from(")"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from(";"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from(":"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from(","), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("."), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("/"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("\\"), GraphemeType::Symbol);
    }

    #[test]
    fn symbol_cjk_punctuation() {
        assert_eq!(GraphemeType::from("，"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("。"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("；"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("："), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("！"), GraphemeType::Symbol);
        assert_eq!(GraphemeType::from("？"), GraphemeType::Symbol);
    }

    #[test]
    fn whitespace_space() {
        assert_eq!(GraphemeType::from(" "), GraphemeType::Whitespace);
    }

    #[test]
    fn whitespace_tab() {
        assert_eq!(GraphemeType::from("\t"), GraphemeType::Whitespace);
    }

    #[test]
    fn whitespace_newline() {
        assert_eq!(GraphemeType::from("\n"), GraphemeType::Whitespace);
    }

    #[test]
    fn whitespace_empty() {
        assert_eq!(GraphemeType::from(""), GraphemeType::Whitespace);
    }

    // --- WordType ---

    #[test]
    fn word_type_keyword_symbol_separator() {
        assert_eq!(
            WordType::from(GraphemeType::Keyword, WordSeparatorType::Symbol),
            WordType::PureKeyword
        );
    }

    #[test]
    fn word_type_keyword_whitespace_separator() {
        assert_eq!(
            WordType::from(GraphemeType::Keyword, WordSeparatorType::Whitespace),
            WordType::Mix
        );
    }

    #[test]
    fn word_type_symbol_whitespace_separator() {
        assert_eq!(
            WordType::from(GraphemeType::Symbol, WordSeparatorType::Whitespace),
            WordType::Mix
        );
    }

    #[test]
    fn word_type_symbol_symbol_separator() {
        assert_eq!(
            WordType::from(GraphemeType::Symbol, WordSeparatorType::Symbol),
            WordType::PureSymbol
        );
    }

    #[test]
    fn word_type_whitespace() {
        assert_eq!(
            WordType::from(GraphemeType::Whitespace, WordSeparatorType::Symbol),
            WordType::None
        );
    }
}
