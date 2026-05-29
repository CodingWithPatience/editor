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
