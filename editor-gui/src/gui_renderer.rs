use editor_core::annotated_string::AnnotatedString;
use editor_core::annotation_type::AnnotationType;
use editor_core::config::ThemeConfig;
use egui::text::LayoutJob;
use egui::{Color32, TextFormat};
use unicode_segmentation::UnicodeSegmentation;

fn rgb(c: Option<[u8; 3]>, default: Color32) -> Color32 {
    c.map(|[r, g, b]| Color32::from_rgb(r, g, b)).unwrap_or(default)
}

/// 从配置解析后的主题颜色。
pub struct ThemeColors {
    pub text: Color32,
    pub background: Color32,
    pub cursor: Color32,
    pub selection_bg: Color32,
    pub search_match_bg: Color32,
    pub search_selected_bg: Color32,
    pub prompt_bg: Color32,
    pub line_number: Color32,
    pub line_number_active: Color32,
    pub keyword: Color32,
    pub string_color: Color32,
    pub comment: Color32,
    pub number: Color32,
    pub type_color: Color32,
    pub known_value: Color32,
    pub char_color: Color32,
    pub lifetime: Color32,
    pub match_color: Color32,
    pub selected_match: Color32,
    pub selection_text: Color32,
}

impl ThemeColors {
    pub fn from_config(theme: &ThemeConfig) -> Self {
        Self {
            text: rgb(theme.text, Color32::from_rgb(210, 210, 210)),
            background: rgb(theme.background, Color32::from_rgb(30, 30, 30)),
            cursor: rgb(theme.cursor_color, Color32::from_rgb(255, 255, 200)),
            selection_bg: rgb(theme.selection_bg, Color32::from_rgb(60, 60, 120)),
            search_match_bg: rgb(theme.search_match_bg, Color32::from_rgb(80, 80, 0)),
            search_selected_bg: rgb(theme.search_selected_bg, Color32::from_rgb(120, 120, 0)),
            prompt_bg: rgb(theme.prompt_bg, Color32::from_rgb(40, 40, 50)),
            line_number: rgb(theme.line_number, Color32::from_rgb(100, 130, 200)),
            line_number_active: rgb(theme.line_number_active, Color32::from_rgb(100, 100, 100)),
            keyword: rgb(theme.keyword, Color32::from_rgb(100, 149, 237)),
            string_color: rgb(theme.string_color, Color32::from_rgb(255, 179, 102)),
            comment: rgb(theme.comment, Color32::from_rgb(87, 166, 74)),
            number: rgb(theme.number, Color32::from_rgb(255, 99, 71)),
            type_color: rgb(theme.type_color, Color32::from_rgb(175, 225, 175)),
            known_value: rgb(theme.known_value, Color32::from_rgb(195, 175, 225)),
            char_color: rgb(theme.char_color, Color32::from_rgb(255, 191, 225)),
            lifetime: rgb(theme.lifetime, Color32::from_rgb(102, 205, 170)),
            match_color: Color32::from_rgb(255, 255, 255),
            selected_match: Color32::from_rgb(255, 200, 80),
            selection_text: Color32::from_rgb(255, 255, 255),
        }
    }
}

pub fn annotation_to_color(annotation_type: AnnotationType, theme: &ThemeColors) -> Color32 {
    match annotation_type {
        AnnotationType::Match => theme.match_color,
        AnnotationType::SelectedMatch => theme.selected_match,
        AnnotationType::Number => theme.number,
        AnnotationType::Keyword => theme.keyword,
        AnnotationType::Type => theme.type_color,
        AnnotationType::KnownValue => theme.known_value,
        AnnotationType::Char => theme.char_color,
        AnnotationType::LifetimeSpecifier => theme.lifetime,
        AnnotationType::Comment => theme.comment,
        AnnotationType::String => theme.string_color,
        AnnotationType::LineNumber => theme.line_number,
        AnnotationType::LineNumberSection => theme.line_number_active,
        AnnotationType::Selection => theme.selection_text,
    }
}

// 默认颜色常量（仅用于 build_layout_job，当前未被调用）
#[allow(dead_code)]
pub const DEFAULT_TEXT_COLOR: Color32 = Color32::from_rgb(210, 210, 210);
#[allow(dead_code)]
pub const EDITOR_BG_COLOR: Color32 = Color32::from_rgb(30, 30, 30);
#[allow(dead_code)]
pub const CURSOR_COLOR: Color32 = Color32::from_rgb(255, 255, 200);
#[allow(dead_code)]
pub const SELECTION_BG_COLOR: Color32 = Color32::from_rgb(60, 60, 120);
#[allow(dead_code)]
pub const SEARCH_MATCH_BG_COLOR: Color32 = Color32::from_rgb(80, 80, 0);
#[allow(dead_code)]
pub const SEARCH_SELECTED_BG_COLOR: Color32 = Color32::from_rgb(120, 120, 0);
#[allow(dead_code)]
pub const PROMPT_BG_COLOR: Color32 = Color32::from_rgb(40, 40, 50);

/// 行号区域固定字符宽度（与 editor-term GUTTER_WIDTH=6 一致）。
pub const GUTTER_CHARS: usize = 6;

/// 构建带行号、语法高亮和选择高亮的 LayoutJob。
/// `sel_range`: (start_line, start_graheme, end_line, end_graheme)
/// 坐标使用 grapheme 索引（非 char 索引），与 Buffer API 一致。
#[allow(dead_code)]
pub fn build_layout_job(
    lines: &[(usize, AnnotatedString)],
    cursor_line: usize,
    sel_range: Option<(usize, usize, usize, usize)>,
    theme: &ThemeColors,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let gutter_fmt = GUTTER_CHARS - 1;

    for (line_idx, annotated_string) in lines {
        let is_cursor = *line_idx == cursor_line;

        // --- 行号 ---
        let number_color = if is_cursor {
            theme.line_number
        } else {
            theme.line_number_active
        };
        let number_text = format!("{:>w$} ", line_idx + 1, w = gutter_fmt);
        job.append(
            &number_text,
            0.0,
            TextFormat {
                color: number_color,
                background: theme.background,
                ..Default::default()
            },
        );

        // --- 选择范围计算 ---
        let in_sel = sel_range
            .map(|(sl, _, el, _)| *line_idx >= sl && *line_idx <= el)
            .unwrap_or(false);
        let (seg_g_start, seg_g_end) = if in_sel {
            sel_range
                .map(|(sl, sg, el, eg)| {
                    let s = if *line_idx == sl { sg } else { 0 };
                    let e = if *line_idx == el { eg } else { usize::MAX };
                    (s, e)
                })
                .unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        // --- 文本内容 ---
        let mut grapheme_idx = 0usize;
        for part in annotated_string {
            let color = part
                .annotated_type
                .map(|at| annotation_to_color(at, theme))
                .unwrap_or(theme.text);
            let part_graphemes: Vec<&str> = part.string.graphemes(true).collect();
            let part_g_len = part_graphemes.len();

            let bg =
                if in_sel && grapheme_idx + part_g_len > seg_g_start && grapheme_idx < seg_g_end {
                    theme.selection_bg
                } else {
                    theme.background
                };

            job.append(
                part.string,
                0.0,
                TextFormat {
                    color,
                    background: bg,
                    ..Default::default()
                },
            );
            grapheme_idx += part_g_len;
        }

        // --- 行尾填充（保持对齐） ---
        job.append(
            "\n",
            0.0,
            TextFormat {
                color: theme.text,
                background: theme.background,
                ..Default::default()
            },
        );
    }
    job
}
