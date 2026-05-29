use editor_core::annotated_string::AnnotatedString;
use editor_core::annotation_type::AnnotationType;
use egui::text::LayoutJob;
use egui::{Color32, TextFormat};
use unicode_segmentation::UnicodeSegmentation;

pub fn annotation_to_color(annotation_type: AnnotationType) -> Color32 {
    match annotation_type {
        AnnotationType::Match => Color32::from_rgb(255, 255, 255),
        AnnotationType::SelectedMatch => Color32::from_rgb(255, 200, 80),
        AnnotationType::Number => Color32::from_rgb(255, 99, 71),
        AnnotationType::Keyword => Color32::from_rgb(100, 149, 237),
        AnnotationType::Type => Color32::from_rgb(175, 225, 175),
        AnnotationType::KnownValue => Color32::from_rgb(195, 175, 225),
        AnnotationType::Char => Color32::from_rgb(255, 191, 225),
        AnnotationType::LifetimeSpecifier => Color32::from_rgb(102, 205, 170),
        AnnotationType::Comment => Color32::from_rgb(87, 166, 74),
        AnnotationType::String => Color32::from_rgb(255, 179, 102),
        AnnotationType::LineNumber => Color32::from_rgb(100, 130, 200),
        AnnotationType::LineNumberSection => Color32::from_rgb(100, 100, 100),
        AnnotationType::Selection => Color32::from_rgb(255, 255, 255),
    }
}

pub const DEFAULT_TEXT_COLOR: Color32 = Color32::from_rgb(210, 210, 210);
pub const EDITOR_BG_COLOR: Color32 = Color32::from_rgb(30, 30, 30);
// pub const GUTTER_BG_COLOR: Color32 = Color32::from_rgb(50, 50, 50);
pub const CURSOR_COLOR: Color32 = Color32::from_rgb(255, 255, 200);
pub const SELECTION_BG_COLOR: Color32 = Color32::from_rgb(60, 60, 120);
pub const SEARCH_MATCH_BG_COLOR: Color32 = Color32::from_rgb(80, 80, 0);
pub const SEARCH_SELECTED_BG_COLOR: Color32 = Color32::from_rgb(120, 120, 0);
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
) -> LayoutJob {
    let mut job = LayoutJob::default();
    // 固定行号宽度：数字右对齐在 GUTTER_CHARS-1 列内，加 1 空格分隔符
    let gutter_fmt = GUTTER_CHARS - 1;

    for (line_idx, annotated_string) in lines {
        let is_cursor = *line_idx == cursor_line;

        // --- 行号 ---
        let number_color = if is_cursor {
            annotation_to_color(AnnotationType::LineNumber)
        } else {
            annotation_to_color(AnnotationType::LineNumberSection)
        };
        let number_text = format!("{:>w$} ", line_idx + 1, w = gutter_fmt);
        job.append(
            &number_text,
            0.0,
            TextFormat {
                color: number_color,
                background: EDITOR_BG_COLOR,
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
                .map(annotation_to_color)
                .unwrap_or(DEFAULT_TEXT_COLOR);
            // 使用 grapheme 分段粒度精确计数
            let part_graphemes: Vec<&str> = part.string.graphemes(true).collect();
            let part_g_len = part_graphemes.len();

            let bg =
                if in_sel && grapheme_idx + part_g_len > seg_g_start && grapheme_idx < seg_g_end {
                    SELECTION_BG_COLOR
                } else {
                    EDITOR_BG_COLOR
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
                color: DEFAULT_TEXT_COLOR,
                background: EDITOR_BG_COLOR,
                ..Default::default()
            },
        );
    }
    job
}
