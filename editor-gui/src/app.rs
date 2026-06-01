use crate::gui_renderer::{annotation_to_color, ThemeColors, GUTTER_CHARS};
use crate::keymap::egui_to_key;
use editor_core::annotation_type::AnnotationType;
use editor_core::buffer::Buffer;
use editor_core::command::edit::Edit;
use editor_core::command::mode::{CommandValue, ComputeModeAction, Mode};
use editor_core::command::move_command::Move;
use editor_core::command::system::System;
use editor_core::config::EditorConfig;
use editor_core::highlighter::Highlighter;
use editor_core::key::Key;
use editor_core::prelude::Location;
use egui::{pos2, vec2, Color32, Rect};
use std::cmp::Ordering;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

const MAX_UNDO_DEPTH: usize = 1000;
const QUIT_TIMES: u8 = 3;
const MSG_TTL_SECS: u64 = 5;

/// 底部提示栏类型
#[derive(Debug, PartialEq, Clone, Copy)]
enum PromptType {
    None,
    Search,
    SearchBackward,
    CommandMode,
}

#[derive(Clone)]
struct BufferSnapshot {
    lines: Vec<String>,
    cursor: Location,
}

pub struct EditorApp {
    buffer: Buffer,
    filename: Option<String>,
    window_title: String,
    mode: Mode,
    text_location: Location,
    selection_anchor: Option<Location>,
    selection_linewise: bool,
    yank_register: Option<String>,
    yank_register_linewise: bool,
    undo_stack: Vec<BufferSnapshot>,
    redo_stack: Vec<BufferSnapshot>,
    count_prefix: Option<usize>,
    #[allow(dead_code)]
    last_edit: Option<Edit>,
    style_initialized: bool,
    /// Visual 模式下的两键序列 pending 状态
    visual_pending_g: bool,
    visual_pending_find: Option<char>,
    // --- 阶段 5：搜索/命令/消息 ---
    prompt_type: PromptType,
    input_text: String,
    input_focus_requested: bool,
    search_query: Option<String>,
    search_prev_location: Option<Location>,
    message: Option<String>,
    message_time: Option<Instant>,
    quit_times: u8,
    /// IME 是否处于组合状态（Preedit 活跃时过滤 Event::Key）
    ime_composing: bool,
    // --- 阶段 6：配置与视觉优化 ---
    config: EditorConfig,
    theme_colors: ThemeColors,
    cursor_blink_visible: bool,
    cursor_last_toggle: Instant,
}

impl EditorApp {
    /// 创建编辑器应用实例。filename 为 None 时创建空白缓冲区。
    pub fn new(filename: Option<String>) -> Self {
        let config = EditorConfig::load();
        let theme_colors = ThemeColors::from_config(&config.theme);
        let mut app = Self {
            buffer: Buffer::default(),
            filename: filename.clone(),
            window_title: String::new(),
            mode: Mode::Normal,
            text_location: Location::default(),
            selection_anchor: None,
            selection_linewise: false,
            yank_register: None,
            yank_register_linewise: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            count_prefix: None,
            last_edit: None,
            style_initialized: false,
            visual_pending_g: false,
            visual_pending_find: None,
            prompt_type: PromptType::None,
            input_text: String::new(),
            input_focus_requested: false,
            search_query: None,
            search_prev_location: None,
            message: None,
            message_time: None,
            quit_times: 0,
            ime_composing: false,
            config,
            theme_colors,
            cursor_blink_visible: true,
            cursor_last_toggle: Instant::now(),
        };
        if let Some(ref name) = filename {
            if let Ok(buf) = Buffer::load(name) {
                app.buffer = buf;
            }
        }
        app.window_title = app.compute_window_title();
        app
    }
    fn compute_window_title(&self) -> String {
        match &self.filename {
            Some(name) => format!("{} - Editor GUI", name),
            None => "Editor GUI - [No Name]".into(),
        }
    }

    // ---- 选择范围 ----
    fn selection_range(&self) -> Option<(usize, usize, usize, usize)> {
        let anchor = self.selection_anchor?;
        let cursor = self.text_location;
        if self.selection_linewise {
            let (a, c) = if anchor.line_idx <= cursor.line_idx {
                (anchor, cursor)
            } else {
                (cursor, anchor)
            };
            return Some((
                a.line_idx,
                0,
                c.line_idx,
                self.buffer.grapheme_count(c.line_idx),
            ));
        }
        if anchor.line_idx == cursor.line_idx && anchor.grapheme_idx == cursor.grapheme_idx {
            let gc = self.buffer.grapheme_count(cursor.line_idx);
            if gc == 0 {
                return None;
            }
            let end = cursor.grapheme_idx.saturating_add(1).min(gc);
            return Some((cursor.line_idx, cursor.grapheme_idx, cursor.line_idx, end));
        }
        let cursor_gc = self.buffer.grapheme_count(cursor.line_idx);
        let anchor_gc = self.buffer.grapheme_count(anchor.line_idx);
        match (
            anchor.line_idx.cmp(&cursor.line_idx),
            anchor.grapheme_idx.cmp(&cursor.grapheme_idx),
        ) {
            (Ordering::Less, _) | (Ordering::Equal, Ordering::Less) => Some((
                anchor.line_idx,
                anchor.grapheme_idx,
                cursor.line_idx,
                cursor.grapheme_idx.saturating_add(1).min(cursor_gc),
            )),
            _ => Some((
                cursor.line_idx,
                cursor.grapheme_idx,
                anchor.line_idx,
                anchor.grapheme_idx.saturating_add(1).min(anchor_gc),
            )),
        }
    }

    // ---- 键盘 ----
    fn process_key_events(&mut self, ctx: &egui::Context) -> bool {
        let mut dirty = false;
        let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());

        // 检测当前帧是否存在 IME 活动（非空 Preedit 或 Commit），
        // 解决 Event::Key 先于 ImeEvent::Enabled 到达的时序问题
        let ime_active = events.iter().any(|e| {
            matches!(e, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
                || matches!(e, egui::Event::Ime(egui::ImeEvent::Commit(_)))
        });

        for e in &events {
            match e {
                // IME 状态追踪
                egui::Event::Ime(ime_event) => {
                    match ime_event {
                        egui::ImeEvent::Enabled => {
                            self.ime_composing = true;
                        }
                        egui::ImeEvent::Disabled => {
                            self.ime_composing = false;
                        }
                        egui::ImeEvent::Commit(text) => {
                            self.ime_composing = false;
                            self.cursor_blink_visible = true;
                            self.cursor_last_toggle = Instant::now();
                            if self.prompt_type == PromptType::None
                                && self.mode == Mode::Insert
                            {
                                // Insert 模式：IME 文本写入 buffer
                                for ch in text.chars() {
                                    self.buffer.insert_char(ch, self.text_location);
                                    self.text_location.grapheme_idx += 1;
                                }
                                dirty = true;
                            } else if matches!(
                                self.prompt_type,
                                PromptType::Search | PromptType::SearchBackward
                            ) {
                                // 搜索模式：TextEdit 已更新 input_text，触发增量搜索
                                self.do_incremental_search();
                                dirty = true;
                            }
                        }
                        _ => {}
                    }
                }
                // IME 活跃帧内跳过 Event::Key 和 Event::Text，
                // 避免原始按键/文本与 IME Commit 重复写入
                egui::Event::Key { .. }
                    if self.ime_composing || ime_active =>
                {
                }
                egui::Event::Text(_)
                    if self.ime_composing || ime_active =>
                {
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if self.prompt_type != PromptType::None {
                        // 提示栏活跃时：拦截 Enter/Escape，其余交给 TextEdit
                        if *key == egui::Key::Enter {
                            self.submit_prompt();
                            dirty = true;
                        } else if *key == egui::Key::Escape {
                            self.dismiss_prompt();
                            dirty = true;
                        }
                    } else if let Some(k) = egui_to_key(*key, *modifiers) {
                        // 按键时重置光标闪烁为可见
                        self.cursor_blink_visible = true;
                        self.cursor_last_toggle = Instant::now();
                        let mode_before = self.mode;
                        dirty |= self.handle_key(k);
                        // 模式切换时控制 IME 开关
                        if self.mode != mode_before {
                            let ime_on = self.mode == Mode::Insert;
                            ctx.send_viewport_cmd(egui::ViewportCommand::IMEAllowed(ime_on));
                            if !ime_on {
                                self.ime_composing = false;
                            }
                        }
                    }
                }
                // 搜索提示栏活跃时：捕获文本输入事件用于增量搜索
                egui::Event::Text(_text)
                    if matches!(
                        self.prompt_type,
                        PromptType::Search | PromptType::SearchBackward
                    ) =>
                {
                    // TextEdit 已经将文本插入 input_text，触发增量搜索
                    self.do_incremental_search();
                    dirty = true;
                }
                _ => {}
            }
        }
        if dirty {
            ctx.request_repaint();
        }
        dirty
    }

    fn handle_key(&mut self, key: Key) -> bool {
        if self.mode == Mode::Normal {
            if let Key::Char(ch) = key {
                if ch.is_ascii_digit() && ch != '0' {
                    let d = ch.to_digit(10).unwrap_or(0) as usize;
                    self.count_prefix = Some(
                        self.count_prefix
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(d),
                    );
                    return false;
                }
            }
        }
        if key == Key::Escape {
            self.count_prefix = None;
            self.visual_pending_g = false;
            self.visual_pending_find = None;
            if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                self.selection_anchor = None;
                self.mode = Mode::Normal;
                return true;
            }
        }

        let in_visual = matches!(self.mode, Mode::Visual | Mode::VisualLine);

        // Visual 模式两键序列预处理
        if in_visual {
            // gg: 两个 g → FirstLine
            if key == Key::Char('g') {
                if self.visual_pending_g {
                    self.visual_pending_g = false;
                    let n = self.count_prefix.take().unwrap_or(1);
                    for _ in 0..n {
                        self.handle_move(Move::FirstLine);
                    }
                    return true;
                }
                self.visual_pending_g = true;
                return false;
            }
            self.visual_pending_g = false;

            // f/F/t/T: 第二个按键 → FindChar/TillChar
            if let Some(fkind) = self.visual_pending_find {
                self.visual_pending_find = None;
                let ch = match key {
                    Key::Char(c) | Key::Shift(c) => c,
                    _ => {
                        return false;
                    }
                };
                let cmd = match fkind {
                    'f' => Move::FindChar(ch),
                    'F' => Move::FindCharBackward(ch),
                    't' => Move::TillChar(ch),
                    'T' => Move::TillCharBackward(ch),
                    _ => return false,
                };
                let n = self.count_prefix.take().unwrap_or(1);
                for _ in 0..n {
                    self.handle_move(cmd);
                }
                return true;
            }
            match key {
                Key::Char('f') => {
                    self.visual_pending_find = Some('f');
                    return false;
                }
                Key::Shift('F') => {
                    self.visual_pending_find = Some('F');
                    return false;
                }
                Key::Char('t') => {
                    self.visual_pending_find = Some('t');
                    return false;
                }
                Key::Shift('T') => {
                    self.visual_pending_find = Some('T');
                    return false;
                }
                Key::Char('o') => {
                    if let Some(a) = self.selection_anchor {
                        self.selection_anchor = Some(self.text_location);
                        self.text_location = a;
                        return true;
                    }
                    return false;
                }
                _ => {}
            }
        }

        let mode_before = self.mode;
        let mode_action = self.mode.compute_mode_action(key);
        let clear_selection = if self.mode != mode_action.mode {
            if mode_action.mode == Mode::Insert {
                self.take_snapshot();
            }
            let old_mode = self.mode;
            self.mode = mode_action.mode;
            if matches!(self.mode, Mode::Visual | Mode::VisualLine)
                && !matches!(old_mode, Mode::Visual | Mode::VisualLine)
            {
                self.selection_anchor = Some(self.text_location);
                self.selection_linewise = self.mode == Mode::VisualLine;
            }
            let should_clear = matches!(old_mode, Mode::Visual | Mode::VisualLine)
                && !matches!(self.mode, Mode::Visual | Mode::VisualLine);
            if self.mode == Mode::Normal || old_mode == Mode::Insert {
                self.snap_cursor();
            }
            should_clear
        } else {
            false
        };

        let mut dirty = false;
        if let Some(cl) = mode_action.command_list {
            let n = self.count_prefix.take().unwrap_or(1);
            for _ in 0..n {
                for &c in &cl {
                    if self.process_command(c) {
                        dirty = true;
                    }
                }
            }
        }

        // System 命令执行后恢复原模式（Save/Quit/Search 等不需要改变当前编辑模式）
        if matches!(self.mode, Mode::System(_)) {
            self.mode = mode_before;
        }

        // 在命令执行之后清除selection信息，确保DeleteSelection等命令能正确获取selection
        if clear_selection {
            self.selection_anchor = None;
        }

        if self.mode != mode_before {
            dirty = true;
        }
        dirty
    }

    fn process_command(&mut self, c: editor_core::command::Command) -> bool {
        match c {
            editor_core::command::Command::Edit(e) => self.handle_edit(e),
            editor_core::command::Command::Move(m) => self.handle_move(m),
            editor_core::command::Command::System(s) => self.handle_system(s),
        }
    }

    fn handle_system(&mut self, s: System) -> bool {
        match s {
            System::Save => {
                self.handle_save();
                true
            }
            System::Quit => {
                self.handle_quit();
                true
            }
            System::Search => {
                self.enter_search(false);
                true
            }
            System::SearchBackward => {
                self.enter_search(true);
                true
            }
            System::SearchWordForward => {
                self.search_word_under_cursor(true);
                true
            }
            System::SearchWordBackward => {
                self.search_word_under_cursor(false);
                true
            }
            System::CommandMode => {
                self.enter_command_mode();
                true
            }
            System::Dismiss => {
                self.dismiss_prompt();
                true
            }
            _ => false,
        }
    }

    // ---- 编辑 ----
    fn handle_edit(&mut self, cmd: Edit) -> bool {
        match cmd {
            Edit::Insert(ch) => {
                self.buffer.insert_char(ch, self.text_location);
                self.text_location.grapheme_idx += 1;
                true
            }
            Edit::InsertNewline => {
                self.buffer.insert_newline(self.text_location);
                self.text_location.line_idx += 1;
                self.text_location.grapheme_idx = 0;
                true
            }
            Edit::Delete => {
                self.take_snapshot();
                self.buffer.delete(self.text_location);
                self.snap_cursor();
                true
            }
            Edit::DeleteBackward => {
                self.take_snapshot();
                if self.text_location.grapheme_idx > 0 {
                    self.text_location.grapheme_idx -= 1;
                    self.buffer.delete(self.text_location);
                } else if self.text_location.line_idx > 0 {
                    let p = self
                        .buffer
                        .grapheme_count(self.text_location.line_idx.saturating_sub(1));
                    self.text_location.line_idx -= 1;
                    self.text_location.grapheme_idx = p;
                    self.buffer.delete(self.text_location);
                }
                true
            }
            Edit::DeleteLine => {
                self.take_snapshot();
                if let Some(d) = self.buffer.delete_line(self.text_location.line_idx) {
                    self.yank_register = Some(d);
                    self.yank_register_linewise = true;
                }
                self.snap_cursor();
                true
            }
            Edit::DeleteToEnd => {
                self.take_snapshot();
                let m = self.buffer.grapheme_count(self.text_location.line_idx);
                for _ in self.text_location.grapheme_idx..m {
                    self.buffer.delete(self.text_location);
                }
                self.snap_cursor();
                true
            }
            Edit::YankLine => {
                if let Some(c) = self.buffer.get_line_content(self.text_location.line_idx) {
                    self.yank_register = Some(c);
                    self.yank_register_linewise = true;
                }
                false
            }
            Edit::Paste => {
                self.take_snapshot();
                if !self.yank_register_linewise {
                    // 非 linewise：p 命令粘贴到光标字符之后
                    let gc = self.buffer.grapheme_count(self.text_location.line_idx);
                    self.text_location.grapheme_idx = self.text_location.grapheme_idx.saturating_add(1).min(gc);
                }
                self.paste(false);
                true
            }
            Edit::PasteAbove => {
                self.take_snapshot();
                self.paste(true);
                true
            }
            Edit::Undo => {
                self.undo();
                true
            }
            Edit::Redo => {
                self.redo();
                true
            }
            Edit::DeleteToNextWordStart => {
                self.take_snapshot();
                let li = self.text_location.line_idx;
                let gc = self.buffer.grapheme_count(li);
                let t = self
                    .buffer
                    .find_next_word_start_on_line(li, self.text_location.grapheme_idx);
                let n = t
                    .unwrap_or(gc)
                    .saturating_sub(self.text_location.grapheme_idx);
                for _ in 0..n {
                    self.buffer.delete(self.text_location);
                }
                self.snap_cursor();
                true
            }
            Edit::DeleteWholeLine => {
                self.take_snapshot();
                let m = self.buffer.grapheme_count(self.text_location.line_idx);
                let l = self.text_location.line_idx;
                for _ in 0..m {
                    self.buffer.delete(Location {
                        line_idx: l,
                        grapheme_idx: 0,
                    });
                }
                self.snap_cursor();
                true
            }
            Edit::DeleteToLineStart => {
                self.take_snapshot();
                let n = self.text_location.grapheme_idx;
                self.text_location.grapheme_idx = 0;
                for _ in 0..n {
                    self.buffer.delete(self.text_location);
                }
                true
            }
            Edit::JoinLines => {
                self.take_snapshot();
                if self.text_location.line_idx + 1 < self.buffer.height() {
                    let e = self.buffer.grapheme_count(self.text_location.line_idx);
                    self.text_location.grapheme_idx = e;
                    self.buffer.delete(self.text_location);
                }
                self.snap_cursor();
                true
            }
            Edit::DeleteWord => {
                self.take_snapshot();
                self.text_location = self.buffer.delete_word_forward(self.text_location);
                self.snap_cursor();
                true
            }
            Edit::DeleteWordBackward => {
                self.take_snapshot();
                self.text_location = self.buffer.delete_word_backward(self.text_location);
                self.snap_cursor();
                true
            }
            Edit::DeleteRangeToMove(move_cmd) => {
                self.take_snapshot();
                let start = self.text_location;
                self.handle_move(move_cmd);
                let end = self.text_location;
                // e/E 命令移动到单词末尾字符（inclusive），需要将 end 向后移动一位
                let actual_end = if matches!(
                    move_cmd,
                    Move::EndOfWordSplitBySymbol | Move::EndOfWordSplitByWhitespace
                ) {
                    Location {
                        line_idx: end.line_idx,
                        grapheme_idx: end
                            .grapheme_idx
                            .saturating_add(1)
                            .min(self.buffer.grapheme_count(end.line_idx)),
                    }
                } else {
                    end
                };
                self.delete_range(
                    start.line_idx,
                    start.grapheme_idx,
                    actual_end.line_idx,
                    actual_end.grapheme_idx,
                );
                self.text_location = start;
                self.snap_cursor();
                true
            }
            Edit::ChangeRangeToMove(move_cmd) => {
                self.take_snapshot();
                let start = self.text_location;
                self.handle_move(move_cmd);
                let end = self.text_location;
                // e/E 命令移动到单词末尾字符（inclusive），需要将 end 向后移动一位
                let actual_end = if matches!(
                    move_cmd,
                    Move::EndOfWordSplitBySymbol | Move::EndOfWordSplitByWhitespace
                ) {
                    Location {
                        line_idx: end.line_idx,
                        grapheme_idx: end
                            .grapheme_idx
                            .saturating_add(1)
                            .min(self.buffer.grapheme_count(end.line_idx)),
                    }
                } else {
                    end
                };
                self.delete_range(
                    start.line_idx,
                    start.grapheme_idx,
                    actual_end.line_idx,
                    actual_end.grapheme_idx,
                );
                self.text_location = start;
                self.snap_cursor();
                self.mode = Mode::Insert;
                true
            }
            Edit::DeleteSelection => {
                self.take_snapshot();
                if let Some((sl, sg, el, eg)) = self.selection_range() {
                    self.yank_selection(sl, sg, el, eg);
                    self.delete_range(sl, sg, el, eg);
                }
                self.selection_anchor = None;
                self.snap_cursor();
                self.mode = Mode::Normal;
                true
            }
            Edit::YankSelection => {
                if let Some((sl, sg, el, eg)) = self.selection_range() {
                    self.yank_selection(sl, sg, el, eg);
                }
                self.selection_anchor = None;
                self.mode = Mode::Normal;
                true
            }
            Edit::ReplaceWithYank => {
                self.take_snapshot();
                if let Some((sl, sg, el, eg)) = self.selection_range() {
                    self.delete_range(sl, sg, el, eg);
                    self.text_location = Location {
                        line_idx: sl,
                        grapheme_idx: sg,
                    };
                    self.paste(false);
                }
                self.selection_anchor = None;
                self.mode = Mode::Normal;
                true
            }
            Edit::Indent => {
                self.take_snapshot();
                if let Some((sl, _, el, _)) = self.selection_range() {
                    for li in sl..=el {
                        self.buffer.insert_char(
                            ' ',
                            Location {
                                line_idx: li,
                                grapheme_idx: 0,
                            },
                        );
                        self.buffer.insert_char(
                            ' ',
                            Location {
                                line_idx: li,
                                grapheme_idx: 0,
                            },
                        );
                    }
                    self.selection_anchor = None;
                    self.mode = Mode::Normal;
                } else {
                    let at = Location {
                        line_idx: self.text_location.line_idx,
                        grapheme_idx: 0,
                    };
                    self.buffer.insert_char(' ', at);
                    self.buffer.insert_char(' ', at);
                }
                true
            }
            Edit::Deindent => {
                self.take_snapshot();
                if let Some((sl, _, el, _)) = self.selection_range() {
                    for li in sl..=el {
                        let line = self.buffer.get_line_content(li).unwrap_or_default();
                        let n = line
                            .graphemes(true)
                            .take_while(|g| *g == " ")
                            .count()
                            .min(2);
                        for _ in 0..n {
                            self.buffer.delete(Location {
                                line_idx: li,
                                grapheme_idx: 0,
                            });
                        }
                    }
                    self.selection_anchor = None;
                    self.mode = Mode::Normal;
                } else {
                    let line = self
                        .buffer
                        .get_line_content(self.text_location.line_idx)
                        .unwrap_or_default();
                    let n = line
                        .graphemes(true)
                        .take_while(|g| *g == " ")
                        .count()
                        .min(2);
                    for _ in 0..n {
                        self.buffer.delete(Location {
                            line_idx: self.text_location.line_idx,
                            grapheme_idx: 0,
                        });
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn handle_move(&mut self, cmd: Move) -> bool {
        match cmd {
            Move::Left => {
                self.text_location.grapheme_idx = self.text_location.grapheme_idx.saturating_sub(1);
            }
            Move::Right => {
                self.text_location.grapheme_idx = self.text_location.grapheme_idx.saturating_add(1);
            }
            Move::Up => {
                self.text_location.line_idx = self.text_location.line_idx.saturating_sub(1);
            }
            Move::Down => {
                self.text_location.line_idx = self.text_location.line_idx.saturating_add(1);
            }
            Move::StartOfLine => {
                self.text_location.grapheme_idx = 0;
            }
            Move::EndOfLine => {
                self.text_location.grapheme_idx =
                    self.buffer.grapheme_count(self.text_location.line_idx);
            }
            Move::FirstLine => {
                self.text_location.line_idx = 0;
            }
            Move::LastLine => {
                self.text_location.line_idx = self.buffer.height().saturating_sub(1);
            }
            Move::FirstVisibleCharOfLine => {
                self.text_location.grapheme_idx = self
                    .buffer
                    .first_visible_of_line_grapheme_idx(self.text_location.line_idx);
            }
            Move::NextWordSplitBySymbol => {
                self.text_location = self.buffer.next_word_location_start(
                    &self.text_location,
                    editor_core::prelude::WordSeparatorType::Symbol,
                );
            }
            Move::PrevWordSplitBySymbol => {
                self.text_location = self.buffer.current_or_prev_word_location_start(
                    &self.text_location,
                    editor_core::prelude::WordSeparatorType::Symbol,
                );
            }
            Move::EndOfWordSplitBySymbol => {
                self.text_location = self.buffer.current_or_next_word_location_end(
                    &self.text_location,
                    editor_core::prelude::WordSeparatorType::Symbol,
                );
            }
            Move::NextWordSplitByWhitespace => {
                self.text_location = self.buffer.next_word_location_start(
                    &self.text_location,
                    editor_core::prelude::WordSeparatorType::Whitespace,
                );
            }
            Move::PrevWordSplitByWhitespace => {
                self.text_location = self.buffer.current_or_prev_word_location_start(
                    &self.text_location,
                    editor_core::prelude::WordSeparatorType::Whitespace,
                );
            }
            Move::EndOfWordSplitByWhitespace => {
                self.text_location = self.buffer.current_or_next_word_location_end(
                    &self.text_location,
                    editor_core::prelude::WordSeparatorType::Whitespace,
                );
            }
            Move::PageUp => {
                self.text_location.line_idx = self.text_location.line_idx.saturating_sub(24);
            }
            Move::PageDown => {
                self.text_location.line_idx = self.text_location.line_idx.saturating_add(24);
            }
            Move::HalfPageUp => {
                self.text_location.line_idx = self.text_location.line_idx.saturating_sub(12);
            }
            Move::HalfPageDown => {
                self.text_location.line_idx = self.text_location.line_idx.saturating_add(12);
            }
            Move::SearchNext => {
                self.search_next();
            }
            Move::SearchPrev => {
                self.search_prev();
            }
            _ => {}
        }
        self.snap_cursor();
        matches!(self.mode, Mode::Visual | Mode::VisualLine)
    }

    // ---- 选择操作 ----
    fn delete_range(&mut self, sl: usize, sg: usize, el: usize, eg: usize) {
        // 规范化参数，确保 start <= end
        let (sl, sg, el, eg) = if sl > el || (sl == el && sg > eg) {
            (el, eg, sl, sg)
        } else {
            (sl, sg, el, eg)
        };
        for li in (sl..=el).rev() {
            let s = if li == sl { sg } else { 0 };
            let e = if li == el {
                eg
            } else {
                self.buffer.grapheme_count(li)
            };
            for _ in s..e {
                self.buffer.delete(Location {
                    line_idx: li,
                    grapheme_idx: s,
                });
            }
            if li > sl && self.buffer.grapheme_count(li) == 0 {
                self.buffer.delete_line(li);
            }
        }
        self.text_location = Location {
            line_idx: sl,
            grapheme_idx: sg,
        };
    }
    fn yank_selection(&mut self, sl: usize, sg: usize, el: usize, eg: usize) {
        let mut p = Vec::new();
        for li in sl..=el {
            let c = self.buffer.get_line_content(li).unwrap_or_default();
            let gc = self.buffer.grapheme_count(li);
            let s = if li == sl { sg } else { 0 };
            let e = if li == el { eg.min(gc) } else { gc };
            // 将grapheme索引转换为byte索引来切片字符串
            let byte_s = self.buffer.grapheme_to_byte(li, s.min(gc));
            let byte_e = self.buffer.grapheme_to_byte(li, e.min(gc));
            let bytes = c.as_bytes();
            if byte_s <= byte_e && byte_s <= bytes.len() && byte_e <= bytes.len() {
                p.push(String::from_utf8_lossy(&bytes[byte_s..byte_e]).to_string());
            } else {
                p.push(String::new());
            }
        }
        self.yank_register = Some(p.join("\n"));
        self.yank_register_linewise = self.selection_linewise;
    }

    // ---- 光标/滚动/粘贴/撤销 ----
    fn snap_cursor(&mut self) {
        let h = self.buffer.height();
        if h == 0 {
            self.text_location = Location::default();
            return;
        }
        if self.text_location.line_idx >= h {
            self.text_location.line_idx = h.saturating_sub(1);
        }
        let gc = self.buffer.grapheme_count(self.text_location.line_idx);
        if self.text_location.grapheme_idx > gc {
            self.text_location.grapheme_idx = gc;
        }
        if self.mode == Mode::Normal && self.text_location.grapheme_idx == gc && gc > 0 {
            self.text_location.grapheme_idx -= 1;
        }
    }
    fn paste(&mut self, above: bool) {
        if let Some(ref t) = self.yank_register {
            if self.yank_register_linewise {
                if above {
                    self.text_location.grapheme_idx = 0;
                } else if self.text_location.line_idx == self.buffer.last_line_idx() {
                    self.text_location.grapheme_idx =
                        self.buffer.grapheme_count(self.text_location.line_idx);
                    self.text_location.line_idx = self.buffer.height();
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.text_location.grapheme_idx = 0;
                    self.text_location.line_idx += 1;
                }
                self.buffer.insert_newline(self.text_location);
            }
            for ch in t.chars() {
                if ch == '\n' {
                    self.buffer.insert_newline(self.text_location);
                    self.text_location.line_idx += 1;
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.buffer.insert_char(ch, self.text_location);
                    self.text_location.grapheme_idx += 1;
                }
            }
            self.snap_cursor();
        }
    }
    fn take_snapshot(&mut self) {
        self.redo_stack.clear();
        self.undo_stack.push(BufferSnapshot {
            lines: self.buffer.get_all_lines(),
            cursor: self.text_location,
        });
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
    }
    fn undo(&mut self) {
        if let Some(s) = self.undo_stack.pop() {
            self.redo_stack.push(BufferSnapshot {
                lines: self.buffer.get_all_lines(),
                cursor: self.text_location,
            });
            self.buffer.set_lines(&s.lines);
            self.text_location = s.cursor;
            self.snap_cursor();
        }
    }
    fn redo(&mut self) {
        if let Some(s) = self.redo_stack.pop() {
            self.undo_stack.push(BufferSnapshot {
                lines: self.buffer.get_all_lines(),
                cursor: self.text_location,
            });
            self.buffer.set_lines(&s.lines);
            self.text_location = s.cursor;
            self.snap_cursor();
        }
    }

    // ---- 搜索 ----
    fn enter_search(&mut self, backward: bool) {
        self.prompt_type = if backward {
            PromptType::SearchBackward
        } else {
            PromptType::Search
        };
        self.input_text.clear();
        self.search_prev_location = Some(self.text_location);
        self.input_focus_requested = true;
    }

    fn do_incremental_search(&mut self) {
        if self.input_text.is_empty() {
            return;
        }
        let from = self.search_prev_location.unwrap_or(self.text_location);
        let result = if self.prompt_type == PromptType::SearchBackward {
            self.buffer.search_backward(&self.input_text, from)
        } else {
            self.buffer.search_forward(&self.input_text, from)
        };
        if let Some(loc) = result {
            self.text_location = loc;
            self.snap_cursor();
        }
    }

    fn search_next(&mut self) {
        if let Some(ref query) = self.search_query {
            let from = Location {
                line_idx: self.text_location.line_idx,
                grapheme_idx: self.text_location.grapheme_idx.saturating_add(1),
            };
            if let Some(loc) = self.buffer.search_forward(query, from) {
                self.text_location = loc;
            }
        }
    }

    fn search_prev(&mut self) {
        if let Some(ref query) = self.search_query {
            if let Some(loc) = self.buffer.search_backward(query, self.text_location) {
                self.text_location = loc;
            }
        }
    }

    fn search_word_under_cursor(&mut self, forward: bool) {
        if let Some((start, end)) = self.buffer.inner_word_range(&self.text_location) {
            if let Some(line) = self.buffer.get_line_content(start.line_idx) {
                let sb = self.buffer.grapheme_to_byte(start.line_idx, start.grapheme_idx);
                let eb = self.buffer.grapheme_to_byte(end.line_idx, end.grapheme_idx);
                if sb < eb && eb <= line.len() {
                    let word = line[sb..eb].to_string();
                    let from = if forward {
                        Location {
                            line_idx: self.text_location.line_idx,
                            grapheme_idx: self.text_location.grapheme_idx.saturating_add(1),
                        }
                    } else {
                        self.text_location
                    };
                    let result = if forward {
                        self.buffer.search_forward(&word, from)
                    } else {
                        self.buffer.search_backward(&word, from)
                    };
                    self.search_query = Some(word);
                    if let Some(loc) = result {
                        self.text_location = loc;
                    }
                }
            }
        }
    }

    // ---- 命令模式 ----
    fn enter_command_mode(&mut self) {
        self.prompt_type = PromptType::CommandMode;
        self.input_text.clear();
        self.input_focus_requested = true;
    }

    fn execute_command_input(&mut self) {
        let input = self.input_text.trim();
        if input.is_empty() {
            return;
        }
        // 裸 "e" → 打开文件对话框
        if input == "e" {
            self.show_open_dialog();
            return;
        }
        match CommandValue::try_from(input) {
            Ok(cmd) => self.execute_command_value(cmd),
            Err(e) => self.show_message(&e),
        }
    }

    fn execute_command_value(&mut self, cmd: CommandValue) {
        match cmd {
            CommandValue::Quit => {
                if self.buffer.is_dirty() {
                    self.show_message(
                        "Unsaved changes. Use q! to force quit, or wq to save and quit.",
                    );
                } else {
                    self.quit_times = u8::MAX;
                }
            }
            CommandValue::ForceQuit => {
                self.quit_times = u8::MAX;
            }
            CommandValue::Save => {
                if self.buffer.is_file_loaded() {
                    match self.buffer.save() {
                        Ok(()) => {
                            self.quit_times = 0;
                            self.show_message("File saved.");
                        }
                        Err(e) => self.show_message(&format!("Error: {e}")),
                    }
                } else {
                    self.show_save_dialog();
                }
            }
            CommandValue::SaveAs(path) => {
                self.save_as(&path);
            }
            CommandValue::SaveAndQuit => {
                if self.buffer.is_file_loaded() {
                    match self.buffer.save() {
                        Ok(()) => self.quit_times = u8::MAX,
                        Err(e) => self.show_message(&format!("Error: {e}")),
                    }
                } else {
                    self.show_message("No filename.");
                }
            }
            CommandValue::SaveAsAndQuit(path) => match self.buffer.save_as(&path) {
                Ok(()) => self.quit_times = u8::MAX,
                Err(e) => self.show_message(&format!("Error: {e}")),
            },
            CommandValue::Open(path) => {
                self.open_file(&path);
            }
            CommandValue::ForceOpen(path) => {
                self.open_file(&path);
            }
            CommandValue::NoHighlight => {
                self.search_query = None;
            }
            CommandValue::SetLineNumbers(show) => {
                self.show_message(&format!(
                    "Line numbers {}",
                    if show { "on" } else { "off" }
                ));
            }
        }
    }

    fn open_file(&mut self, path: &str) {
        match Buffer::load(path) {
            Ok(buf) => {
                self.buffer = buf;
                self.filename = Some(path.to_string());
                self.text_location = Location::default();
                self.window_title = self.compute_window_title();
                self.search_query = None;
                self.show_message(&format!("Opened {path}"));
            }
            Err(e) => self.show_message(&format!("Error opening {path}: {e}")),
        }
    }

    // ---- 保存/退出 ----
    fn handle_save(&mut self) {
        if self.buffer.is_file_loaded() {
            match self.buffer.save() {
                Ok(()) => {
                    self.quit_times = 0;
                    self.show_message("File saved.");
                }
                Err(e) => self.show_message(&format!("Error: {e}")),
            }
        } else {
            self.show_save_dialog();
        }
    }

    fn save_as(&mut self, path: &str) {
        match self.buffer.save_as(path) {
            Ok(()) => {
                self.filename = Some(path.to_string());
                self.window_title = self.compute_window_title();
                self.quit_times = 0;
                self.show_message(&format!("Saved as {path}"));
            }
            Err(e) => self.show_message(&format!("Error: {e}")),
        }
    }

    fn handle_quit(&mut self) {
        if !self.buffer.is_dirty() {
            self.quit_times = u8::MAX;
        } else {
            self.quit_times += 1;
            if self.quit_times >= QUIT_TIMES {
                self.quit_times = u8::MAX;
            } else {
                self.show_message(&format!(
                    "WARNING! File has unsaved changes. Press Ctrl-Q {} more time(s) to quit.",
                    QUIT_TIMES - self.quit_times
                ));
            }
        }
    }

    fn show_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
        self.message_time = Some(Instant::now());
    }

    fn show_open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().set_title("Open File").pick_file() {
            if let Some(path_str) = path.to_str() {
                let path_owned = path_str.to_string();
                self.open_file(&path_owned);
            }
        }
    }

    fn show_save_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().set_title("Save File As").save_file() {
            if let Some(path_str) = path.to_str() {
                let path_owned = path_str.to_string();
                self.save_as(&path_owned);
            }
        }
    }

    // ---- 提示栏生命周期 ----
    fn dismiss_prompt(&mut self) {
        if matches!(
            self.prompt_type,
            PromptType::Search | PromptType::SearchBackward
        ) {
            if let Some(loc) = self.search_prev_location {
                self.text_location = loc;
            }
        }
        self.prompt_type = PromptType::None;
        self.input_text.clear();
        self.search_prev_location = None;
        self.mode = Mode::Normal;
    }

    fn submit_prompt(&mut self) {
        match self.prompt_type {
            PromptType::Search | PromptType::SearchBackward => {
                self.search_query = if self.input_text.is_empty() {
                    None
                } else {
                    Some(self.input_text.clone())
                };
                self.search_prev_location = None;
            }
            PromptType::CommandMode => {
                self.execute_command_input();
            }
            PromptType::None => {}
        }
        self.prompt_type = PromptType::None;
        self.input_text.clear();
        self.mode = Mode::Normal;
    }

    // ---- 提示栏/消息栏渲染 ----
    fn render_prompt_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let prompt = match self.prompt_type {
            PromptType::Search => "/ ",
            PromptType::SearchBackward => "? ",
            PromptType::CommandMode => ": ",
            PromptType::None => "",
        };

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(prompt).color(Color32::from_rgb(200, 200, 100)),
            );
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.input_text)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(ui.available_width())
                    .frame(false),
            );

            // 首次渲染时请求焦点
            if self.input_focus_requested {
                response.request_focus();
                self.input_focus_requested = false;
            }

            // TextEdit 内容变化时触发增量搜索
            if response.changed()
                && matches!(
                    self.prompt_type,
                    PromptType::Search | PromptType::SearchBackward
                )
            {
                self.do_incremental_search();
                ctx.request_repaint();
            }
        });
    }

    fn render_message_bar(&mut self, ui: &mut egui::Ui) {
        let ttl = std::time::Duration::from_secs(MSG_TTL_SECS);
        let show_msg = self
            .message
            .as_ref()
            .filter(|_| self.message_time.map_or(false, |t| t.elapsed() < ttl));
        if let Some(msg) = show_msg {
            ui.label(egui::RichText::new(msg).color(Color32::from_rgb(200, 200, 100)));
        }
        if self
            .message_time
            .map_or(false, |t| t.elapsed() >= ttl)
        {
            self.message = None;
            self.message_time = None;
        }
    }

    // ---- 模式显示 ----
    fn mode_label(&self) -> &str {
        match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "VISUAL LINE",
            Mode::Command => "COMMAND",
            _ => "NORMAL",
        }
    }
    fn mode_color(&self) -> egui::Color32 {
        match self.mode {
            Mode::Insert => Color32::from_rgb(100, 200, 100),
            Mode::Visual | Mode::VisualLine => Color32::from_rgb(100, 150, 255),
            _ => Color32::from_rgb(180, 180, 180),
        }
    }
    fn status_text(&self) -> String {
        let f = self.filename.as_deref().unwrap_or("[No Name]");
        let l = self.buffer.height();
        let cur = self.text_location.line_idx + 1;
        let col = self.text_location.grapheme_idx + 1;
        let sel = if self.selection_anchor.is_some() {
            if let Some((sl, sg, el, eg)) = self.selection_range() {
                let c = if self.selection_linewise {
                    el - sl + 1
                } else if sl == el {
                    eg - sg
                } else {
                    let mut n = self.buffer.grapheme_count(sl) - sg;
                    for li in sl + 1..el {
                        n += self.buffer.grapheme_count(li);
                    }
                    n += eg;
                    n
                };
                format!(" [{}]", c)
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        format!("{f} - Ln {cur}/{l}, Col {col}{sel}")
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 退出检查
        if self.quit_times == u8::MAX {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if !self.style_initialized {
            let mut s = (*ctx.style()).clone();
            s.override_text_style = Some(egui::TextStyle::Monospace);
            ctx.set_style(s);
            // 加载 CJK 字体支持中文显示
            let mut fonts = egui::FontDefinitions::default();
            let cjk_font_path: Option<&str> = if let Some(ref p) = self.config.font.path {
                Some(p.as_str())
            } else if cfg!(target_os = "windows") {
                Some("C:\\Windows\\Fonts\\msyh.ttc")
            } else if cfg!(target_os = "macos") {
                Some("/System/Library/Fonts/PingFang.ttc")
            } else if cfg!(target_os = "linux") {
                // 常见 Linux 发行版 CJK 字体路径
                [
                    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
                ]
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .copied()
            } else {
                None
            };
            if let Some(font_path) = cjk_font_path {
                if let Ok(font_data) = std::fs::read(font_path) {
                    fonts.font_data.insert(
                        "cjk_font".to_owned(),
                        egui::FontData::from_owned(font_data).into(),
                    );
                    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                        family.push("cjk_font".to_owned());
                    }
                }
            }
            ctx.set_fonts(fonts);
            self.style_initialized = true;
        }
        self.process_key_events(ctx);

        // 底部状态栏（最底部）
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(self.mode_color(), self.mode_label());
                ui.separator();
                ui.label(self.status_text());
            });
        });

        // 提示/消息栏（状态栏上方）
        egui::TopBottomPanel::bottom("prompt_or_message")
            .frame(
                egui::Frame::default()
                    .fill(self.theme_colors.prompt_bg)
                    .inner_margin(4.0),
            )
            .show(ctx, |ui| {
                if self.prompt_type != PromptType::None {
                    self.render_prompt_bar(ui, ctx);
                } else {
                    self.render_message_bar(ui);
                }
            });

        // 中央编辑区域 — 全 Painter 手动渲染
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(self.theme_colors.background))
            .show(ctx, |ui| {
                let font_id = egui::FontId::monospace(self.config.font.size);
                let row_h = ui.fonts(|f| f.row_height(&font_id));
                // 精确字符宽度：layout 测试串并取平均
                let test = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
                let gal = ui.fonts(|f| {
                    f.layout(
                        test.to_string(),
                        font_id.clone(),
                        Color32::WHITE,
                        f32::INFINITY,
                    )
                });
                let cw = gal.rect.width() / test.len() as f32;

                let total = self.buffer.height().max(1);
                let gutter_px = GUTTER_CHARS as f32 * cw;
                let avail_rect = ui.available_rect_before_wrap();
                let panel_h = avail_rect.height();
                let content_h = total as f32 * row_h;
                let panel_left = avail_rect.left();
                let panel_top = avail_rect.top();

                let scroll_id = ui.make_persistent_id("edit_scroll");
                let scroll_out = egui::ScrollArea::vertical()
                    .id_salt(scroll_id)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        // gutter 背景
                        ui.painter().rect_filled(
                            Rect::from_min_size(pos2(0.0, 0.0), vec2(gutter_px, content_h)),
                            0.0,
                            self.theme_colors.background,
                        );
                        // 渲染每一行
                        let sel = self.selection_range();
                        let file_type = self.buffer.get_file_info().get_file_type();
                        // 搜索高亮：搜索模式下用 input_text，确认后用 search_query
                        let search_q = if matches!(
                            self.prompt_type,
                            PromptType::Search | PromptType::SearchBackward
                        ) {
                            if self.input_text.is_empty() {
                                None
                            } else {
                                Some(self.input_text.as_str())
                            }
                        } else {
                            self.search_query.as_deref()
                        };
                        // 光标位于匹配区域内任意位置时标记为选中匹配
                        let selected_match = search_q.and_then(|q| {
                            if q.is_empty() {
                                return None;
                            }
                            let loc = self.text_location;
                            let q_len = q.graphemes(true).count();
                            // 从光标位置向前回溯，查找包含光标的匹配起始位置
                            // 注意：仅检查当前行，不支持跨行匹配的选中检测
                            let mut candidate = loc;
                            for _ in 0..q_len {
                                if let Some(found) = self.buffer.search_forward(q, candidate) {
                                    if found == candidate
                                        && found.line_idx == loc.line_idx
                                        && loc.grapheme_idx >= found.grapheme_idx
                                        && loc.grapheme_idx < found.grapheme_idx.saturating_add(q_len)
                                    {
                                        return Some(found);
                                    }
                                }
                                // 回溯到行首即停止（跨行匹配不在检测范围内）
                                if candidate.grapheme_idx == 0 {
                                    break;
                                }
                                candidate = Location {
                                    line_idx: candidate.line_idx,
                                    grapheme_idx: candidate.grapheme_idx.saturating_sub(1),
                                };
                            }
                            None
                        });
                        let mut hl = Highlighter::new(search_q, selected_match, file_type);
                        let sel_linewise = self.selection_linewise;
                        for li in 0..total {
                            self.buffer.highlight(li, &mut hl);
                            let y = li as f32 * row_h;
                            let is_cursor = li == self.text_location.line_idx;
                            // 行号
                            let ln_color = if is_cursor {
                                self.theme_colors.line_number
                            } else {
                                self.theme_colors.line_number_active
                            };
                            let ln_text = format!("{:>w$} ", li + 1, w = GUTTER_CHARS - 1);
                            ui.fonts(|f| {
                                f.layout(ln_text.clone(), font_id.clone(), ln_color, f32::INFINITY)
                            });
                            ui.painter().text(
                                pos2(0.0, y),
                                egui::Align2::LEFT_TOP,
                                &ln_text,
                                font_id.clone(),
                                ln_color,
                            );
                            // 内容
                            if let Some(annotated) =
                                self.buffer.get_highlight_substring(li, 0..usize::MAX, &hl)
                            {
                                // 计算选择边界 (grapheme 坐标)
                                let (gs, ge) = if let Some((sl, sg, el, eg)) = sel {
                                    if sel_linewise {
                                        if li >= sl && li <= el {
                                            (0, usize::MAX)
                                        } else {
                                            (usize::MAX, 0)
                                        }
                                    } else {
                                        let s = if li == sl {
                                            sg
                                        } else if li > sl {
                                            0
                                        } else {
                                            usize::MAX
                                        };
                                        let e = if li == el {
                                            eg
                                        } else if li >= sl && li < el {
                                            usize::MAX
                                        } else {
                                            0
                                        };
                                        (s, e)
                                    }
                                } else {
                                    (usize::MAX, 0)
                                };
                                let mut gi = 0usize;
                                let mut px = gutter_px;
                                for part in &annotated {
                                    let color = part
                                        .annotated_type
                                        .map(|at| annotation_to_color(at, &self.theme_colors))
                                        .unwrap_or(self.theme_colors.text);
                                    let part_bg = match part.annotated_type {
                                        Some(AnnotationType::Match) => Some(self.theme_colors.search_match_bg),
                                        Some(AnnotationType::SelectedMatch) => Some(self.theme_colors.search_selected_bg),
                                        _ => None,
                                    };
                                    let part_str = part.string;
                                    // 逐 grapheme 渲染以确保精度
                                    for g in part_str.graphemes(true) {
                                        let g_text = g.to_string();
                                        let g_gal = ui.fonts(|f| {
                                            f.layout(
                                                g_text.clone(),
                                                font_id.clone(),
                                                Color32::WHITE,
                                                f32::INFINITY,
                                            )
                                        });
                                        let gw = g_gal.rect.width();
                                        let bg = if sel.is_some() && gi >= gs && gi < ge {
                                            self.theme_colors.selection_bg
                                        } else if let Some(pbg) = part_bg {
                                            pbg
                                        } else {
                                            self.theme_colors.background
                                        };
                                        if bg != self.theme_colors.background {
                                            ui.painter().rect_filled(
                                                Rect::from_min_size(pos2(px, y), vec2(gw, row_h)),
                                                0.0,
                                                bg,
                                            );
                                        }
                                        ui.painter().text(
                                            pos2(px, y),
                                            egui::Align2::LEFT_TOP,
                                            &g_text,
                                            font_id.clone(),
                                            color,
                                        );
                                        gi += 1;
                                        px += gw;
                                    }
                                }
                            }
                        }
                    });

                let scroll_y = scroll_out.state.offset.y;
                let cursor_y = panel_top + self.text_location.line_idx as f32 * row_h - scroll_y;
                // 计算光标 X 坐标：基于实际字符宽度（CJK 字符宽度约为 ASCII 两倍）
                let cursor_x = {
                    let line_content = self
                        .buffer
                        .get_line_content(self.text_location.line_idx)
                        .unwrap_or_default();
                    let byte_idx = self.buffer.grapheme_to_byte(
                        self.text_location.line_idx,
                        self.text_location.grapheme_idx,
                    );
                    let text_before =
                        &line_content[..byte_idx.min(line_content.len())];
                    let gal = ui.fonts(|f| {
                        f.layout(
                            text_before.to_string(),
                            font_id.clone(),
                            Color32::WHITE,
                            f32::INFINITY,
                        )
                    });
                    panel_left + gutter_px + gal.rect.width()
                };
                // 光标闪烁逻辑
                let blink_interval =
                    Duration::from_millis(self.config.cursor.blink_interval_ms);
                let now = Instant::now();
                if matches!(self.mode, Mode::Insert | Mode::Normal)
                    && now.duration_since(self.cursor_last_toggle) >= blink_interval
                {
                    self.cursor_blink_visible = !self.cursor_blink_visible;
                    self.cursor_last_toggle = now;
                }
                // Visual/Command 模式下光标常亮
                if matches!(
                    self.mode,
                    Mode::Visual | Mode::VisualLine | Mode::Command
                ) {
                    self.cursor_blink_visible = true;
                }

                // 绘制光标（提示栏活跃时或闪烁不可见时隐藏编辑区光标）
                if self.prompt_type == PromptType::None
                    && self.cursor_blink_visible
                    && cursor_y >= panel_top - row_h
                    && cursor_y < panel_top + panel_h + row_h
                {
                    match self.mode {
                        Mode::Insert => {
                            ui.painter().rect_filled(
                                Rect::from_min_size(pos2(cursor_x, cursor_y), vec2(2.0, row_h)),
                                0.0,
                                self.theme_colors.cursor,
                            );
                        }
                        _ => {
                            // Normal/Visual 模式光标块宽度：基于当前字符实际宽度
                            let cursor_ch_w = self
                                .buffer
                                .get_line_content(self.text_location.line_idx)
                                .and_then(|lc| {
                                    let gc = self.buffer.grapheme_count(self.text_location.line_idx);
                                    if gc == 0 || self.text_location.grapheme_idx >= gc {
                                        return None;
                                    }
                                    // grapheme_idx + 1 安全：Normal 模式 snap_cursor 保证 idx < gc
                                    let end = self.buffer.grapheme_to_byte(
                                        self.text_location.line_idx,
                                        (self.text_location.grapheme_idx + 1).min(gc),
                                    );
                                    let start = self.buffer.grapheme_to_byte(
                                        self.text_location.line_idx,
                                        self.text_location.grapheme_idx,
                                    );
                                    let end = end.min(lc.len());
                                    if start < end {
                                        let g = &lc[start..end];
                                        let gal = ui.fonts(|f| {
                                            f.layout(
                                                g.to_string(),
                                                font_id.clone(),
                                                Color32::WHITE,
                                                f32::INFINITY,
                                            )
                                        });
                                        Some(gal.rect.width())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(cw);
                            ui.painter().rect_filled(
                                Rect::from_min_size(
                                    pos2(cursor_x, cursor_y),
                                    vec2(cursor_ch_w, row_h),
                                ),
                                0.0,
                                Color32::from_rgba_unmultiplied(255, 255, 200, 180),
                            );
                        }
                    }
                }
                // 滚动同步
                let c_top = self.text_location.line_idx as f32 * row_h;
                let c_bot = c_top + row_h;
                if (c_top < scroll_y || c_bot > scroll_y + panel_h) && panel_h > 0.0 {
                    let target = if c_top < scroll_y {
                        c_top
                    } else {
                        (c_bot - panel_h).max(0.0)
                    };
                    let mut s = scroll_out.state;
                    s.offset.y = target;
                    s.store(ui.ctx(), scroll_id);
                    ctx.request_repaint();
                }
            });

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title.clone()));
        ctx.request_repaint_after(Duration::from_millis(self.config.cursor.blink_interval_ms));
    }
}
