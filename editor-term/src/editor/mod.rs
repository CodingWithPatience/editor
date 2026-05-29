pub(crate) mod terminal;
mod ui_components;

use crate::editor::ui_components::command_bar::CommandBar;
use crate::editor::ui_components::message_bar::MessageBar;
use crate::editor::ui_components::status_bar::StatusBar;
use crate::editor::ui_components::ui_component::UIComponent;
use crate::editor::ui_components::view::View;
use crate::keymap::ComputeModeAction;
use editor_core::command::edit::Edit::InsertNewline;
use editor_core::command::mode::{CommandValue, Mode};
use editor_core::command::move_command::Move as MoveType;
use editor_core::command::move_command::Move::{Down, Left, Right, Up};
use editor_core::command::system::System::{
    CommandMode, Dismiss, Quit, Resize, Save, Search, SearchBackward, SearchWordBackward,
    SearchWordForward,
};
use editor_core::command::Command::{self, Edit, Move, System};
use editor_core::event_source::{EventSource, InputEvent};
use editor_core::key::Key;
use editor_core::line::Line;
use editor_core::prelude::{Position, Size, GUTTER_WIDTH};
use editor_core::renderer::Renderer;
use std::env;

pub const NAME: &str = env!("CARGO_PKG_NAME");

const QUIT_TIMES: u8 = 3;

#[derive(Default, Eq, PartialEq)]
enum PromptType {
    Save,
    Search,
    CommandMode,
    #[default]
    None,
}

impl PromptType {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

pub struct Editor<R: Renderer, E: EventSource> {
    renderer: R,
    event_source: E,
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    prompt_type: PromptType,
    terminal_size: Size,
    title: String,
    quit_times: u8,
    mode: Mode,
    count_prefix: Option<usize>,
    visual_pending_g: bool,
    visual_pending_find: Option<char>,
    visual_pending_textobj: bool,
}

impl<R: Renderer, E: EventSource> Editor<R, E> {
    pub fn new(mut renderer: R, event_source: E) -> Result<Self, std::io::Error> {
        renderer
            .initialize()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut editor = Self {
            renderer,
            event_source,
            should_quit: false,
            view: View::default_with_line_numbers(),
            status_bar: StatusBar::default(),
            message_bar: MessageBar::default(),
            command_bar: CommandBar::default(),
            prompt_type: PromptType::None,
            terminal_size: Size::default(),
            title: String::new(),
            quit_times: 0,
            mode: Mode::default(),
            count_prefix: None,
            visual_pending_g: false,
            visual_pending_find: None,
            visual_pending_textobj: false,
        };
        let size = editor.renderer.size().unwrap_or_default();
        editor.handle_resize_command(size);
        editor.update_message("HELP: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit");
        let args: Vec<String> = env::args().collect();
        if let Some(filename) = args.get(1) {
            debug_assert!(!filename.is_empty());
            if editor.view.load(filename).is_err() {
                editor.update_message(&format!("ERR: Could not open file: {filename}"));
            }
        }
        editor.refresh_status();
        Ok(editor)
    }

    pub fn run(&mut self) {
        loop {
            self.refresh_screen();
            if self.should_quit {
                break;
            }
            match self.event_source.next_event() {
                Some(event) => self.evaluate_event(event),
                None => {
                    #[cfg(not(debug_assertions))]
                    {
                        continue;
                    }
                }
            }
            self.refresh_status();
        }
    }

    fn try_accumulate_count(&mut self, key: Key) -> bool {
        if self.mode == Mode::Normal {
            if let Key::Char(ch) = key {
                if ch.is_ascii_digit() {
                    let digit_u32 = ch.to_digit(10).expect("已通过 is_ascii_digit 检查");
                    #[allow(clippy::as_conversions)]
                    let digit = digit_u32 as usize; // 安全：0-9 必定适合任何位宽的 usize
                    if self.count_prefix.is_some() || digit > 0 {
                        let existing = self.count_prefix.unwrap_or(0);
                        self.count_prefix = Some(existing * 10 + digit);
                        return true;
                    }
                }
            }
        }
        false
    }

    fn evaluate_event(&mut self, event: InputEvent) {
        match event {
            InputEvent::Key(key) => {
                // Handle gg and f/F/t/T in Visual mode
                if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                    if let Key::Char('g') = key {
                        if self.visual_pending_g {
                            self.visual_pending_g = false;
                            let n = self.count_prefix.take().unwrap_or(1);
                            for _ in 0..n {
                                self.view.handle_move_command(MoveType::FirstLine);
                            }
                            self.visual_pending_textobj = false;
                            return;
                        }
                        self.visual_pending_g = true;
                        self.visual_pending_textobj = false;
                        return;
                    }
                    self.visual_pending_g = false;
                    if let Some(fkind) = self.visual_pending_find {
                        match key {
                            Key::Char(ch) | Key::Shift(ch) => {
                                let cmd = match fkind {
                                    'f' => MoveType::FindChar(ch),
                                    'F' => MoveType::FindCharBackward(ch),
                                    't' => MoveType::TillChar(ch),
                                    'T' => MoveType::TillCharBackward(ch),
                                    _ => {
                                        self.visual_pending_find = None;
                                        self.visual_pending_textobj = false;
                                        return;
                                    }
                                };
                                self.visual_pending_find = None;
                                let n = self.count_prefix.take().unwrap_or(1);
                                for _ in 0..n {
                                    self.view.handle_move_command(cmd);
                                }
                                self.visual_pending_textobj = false;
                                return;
                            }
                            _ => {
                                self.visual_pending_find = None;
                                self.visual_pending_textobj = false;
                                return;
                            }
                        }
                    }
                    match key {
                        Key::Char('f') => {
                            self.visual_pending_find = Some('f');
                            self.visual_pending_textobj = false;
                            return;
                        }
                        Key::Shift('F') => {
                            self.visual_pending_find = Some('F');
                            self.visual_pending_textobj = false;
                            return;
                        }
                        Key::Char('t') => {
                            self.visual_pending_find = Some('t');
                            self.visual_pending_textobj = false;
                            return;
                        }
                        Key::Shift('T') => {
                            self.visual_pending_find = Some('T');
                            self.visual_pending_textobj = false;
                            return;
                        }
                        Key::Char('o') => {
                            self.view.swap_selection_ends();
                            self.visual_pending_textobj = false;
                            return;
                        }
                        Key::Char('i') => {
                            self.visual_pending_textobj = true;
                            return;
                        }
                        Key::Char('w') if self.visual_pending_textobj => {
                            self.visual_pending_textobj = false;
                            self.view.select_inner_word();
                            return;
                        }
                        _ => {}
                    }
                    self.visual_pending_textobj = false;
                }
                // Accumulate count prefix in Normal mode
                if self.try_accumulate_count(key) {
                    return;
                }
                // Clear count on Esc
                if key == Key::Escape {
                    self.count_prefix = None;
                    self.visual_pending_find = None;
                }
                let mode_action = self.mode.compute_mode_action(key);
                if self.mode != mode_action.mode {
                    if mode_action.mode == Mode::Insert {
                        self.view.take_snapshot();
                    }
                    self.mode = mode_action.mode;
                    self.view.set_mode(self.mode);
                }
                if let Some(command_list) = mode_action.command_list {
                    let n = self.count_prefix.take().unwrap_or(1);
                    for _ in 0..n {
                        for &command in &command_list {
                            self.process_command(command);
                        }
                    }
                }
            }
            InputEvent::Resize(size) => {
                let command = System(Resize(size));
                self.process_command(command);
            }
        }
    }

    fn process_command(&mut self, command: Command) {
        if let System(Resize(size)) = command {
            self.handle_resize_command(size);
            return;
        }
        match self.prompt_type {
            PromptType::Save => self.process_command_during_save(command),
            PromptType::Search => self.process_command_during_search(command),
            PromptType::CommandMode => self.process_command_during_command_mode(command),
            PromptType::None => self.process_command_no_prompt(command),
        }
    }

    fn handle_save_command(&mut self) {
        if self.view.is_file_loaded() {
            self.save(None);
        } else {
            self.set_prompt(PromptType::Save);
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn handle_quit_command(&mut self) {
        if !self.view.get_status().is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if self.view.get_status().is_modified {
            self.update_message(&format!(
                "WARNING! file has unsaved changes. Press Ctrl-Q {} more times to quit",
                QUIT_TIMES - self.quit_times - 1
            ));
            self.quit_times += 1;
        }
    }

    fn reset_quit_times(&mut self) {
        if self.quit_times > 0 {
            self.quit_times = 0;
            self.update_message("");
        }
    }

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }
        let bottom_bar_row = self.terminal_size.height.saturating_sub(1);
        let in_prompt = self.in_prompt();
        render_components(
            &mut self.renderer,
            &mut self.command_bar,
            &mut self.message_bar,
            &mut self.status_bar,
            &mut self.view,
            self.terminal_size,
            in_prompt,
        );
        let new_caret_pos = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.caret_position_col(),
            }
        } else {
            self.view.caret_position()
        };
        debug_assert!(new_caret_pos.col <= self.terminal_size.width);
        debug_assert!(new_caret_pos.row <= self.terminal_size.height);
        let _ = self.renderer.move_caret_to(new_caret_pos);
        let _ = self.renderer.set_cursor_style(self.mode.cursor_style());
        let _ = self.renderer.show_caret();
        let _ = self.renderer.flush();
    }

    fn refresh_status(&mut self) {
        let status = self.view.get_status();
        let title = format!("{} {NAME}", status.filename);
        self.status_bar.update_status(status);
        if title != self.title && self.renderer.set_title(&title).is_ok() {
            self.title = title;
        }
    }

    fn save(&mut self, filename: Option<&String>) {
        let result = if let Some(name) = filename {
            self.view.save_as(name)
        } else {
            self.view.save()
        };
        if result.is_ok() {
            self.message_bar.update_message("File saved successfully.");
        } else {
            self.message_bar.update_message("Error writing file!");
        }
    }

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;
        self.view.resize(Size {
            width: size.width,
            height: size.height.saturating_sub(2),
        });
        let bar_size = Size {
            height: 1,
            width: size.width.saturating_sub(GUTTER_WIDTH),
        };
        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.resize(bar_size);
    }

    fn search_word_under_cursor(&mut self, forward: bool) {
        if let Some(word) = self.view.get_word_under_cursor() {
            self.view.enter_search();
            if !forward {
                self.view.set_search_backward();
            }
            self.view.search(&word);
            self.view.last_search_query = Some(Line::from(&word));
            self.set_prompt(PromptType::None);
        }
    }

    fn update_message(&mut self, message: &str) {
        self.message_bar.update_message(message);
    }

    fn in_prompt(&self) -> bool {
        !self.prompt_type.is_none()
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        match prompt_type {
            PromptType::Save => self.command_bar.set_prompt("Save as: "),
            PromptType::Search => {
                self.view.enter_search();
                self.command_bar
                    .set_prompt("Search (Esc to cancel), Up or down arrows to navigate: ");
            }
            PromptType::CommandMode => {
                self.command_bar.set_prompt(":");
            }
            PromptType::None => self.message_bar.set_needs_redraw(true),
        }
        self.command_bar.clear_value();
        self.prompt_type = prompt_type;
    }

    fn process_command_during_save(&mut self, command: Command) {
        match command {
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.update_message("Save aborted.");
            }
            Move(Left) => self.command_bar.move_left(),
            Move(Right) => self.command_bar.move_right(),
            Edit(InsertNewline) => {
                let filename = self.command_bar.value();
                self.save(Some(&filename));
                self.set_prompt(PromptType::None);
            }
            Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
            System(
                Save | Resize(_) | Search | SearchBackward | SearchWordForward | SearchWordBackward
                | Quit | CommandMode,
            )
            | Move(_) => {}
        }
    }

    fn process_command_during_search(&mut self, command: Command) {
        match command {
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.view.dismiss_search();
            }
            Edit(InsertNewline) => {
                self.set_prompt(PromptType::None);
                self.view.confirm_search();
            }
            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                self.view.search(&query);
            }
            Move(Left) => self.command_bar.move_left(),
            Move(Right) => self.command_bar.move_right(),
            Move(Down) => self.view.search_next(),
            Move(Up) => self.view.search_prev(),
            System(
                Save | Resize(_) | Search | SearchBackward | SearchWordForward | SearchWordBackward
                | Quit | CommandMode,
            )
            | Move(_) => {}
        }
    }

    fn process_command_during_command_mode(&mut self, command: Command) {
        match command {
            System(Dismiss) => {
                self.set_prompt(PromptType::None);
                self.view.dismiss_command_mode();
            }
            Edit(InsertNewline) => {
                let command = self.command_bar.value();
                self.set_prompt(PromptType::None);
                let result = CommandValue::try_from(command.as_str());
                match result {
                    Ok(command_value) => match command_value {
                        CommandValue::Quit
                        | CommandValue::ForceQuit
                        | CommandValue::SaveAndQuit
                        | CommandValue::SaveAsAndQuit(_) => {
                            if let Some(message) = self.view.execute_command(command_value) {
                                self.update_message(message.as_str());
                            } else {
                                self.should_quit = true;
                            }
                        }
                        _ => {
                            if let Some(message) = self.view.execute_command(command_value) {
                                self.update_message(message.as_str());
                            }
                        }
                    },
                    Err(e) => self.update_message(e.as_str()),
                }
                self.view.dismiss_command_mode();
            }
            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
            }
            Move(Right) => self.command_bar.move_right(),
            Move(Left) => self.command_bar.move_left(),
            System(
                Save | Resize(_) | Search | SearchBackward | SearchWordForward | SearchWordBackward
                | Quit | CommandMode,
            )
            | Move(_) => {}
        }
    }

    fn process_command_no_prompt(&mut self, command: Command) {
        if matches!(command, System(Quit)) {
            self.handle_quit_command();
            return;
        }
        self.reset_quit_times();

        match command {
            System(Quit | Resize(_) | Dismiss) => {
                if !matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                    self.view.clear_selection();
                }
            }
            System(Search) => self.set_prompt(PromptType::Search),
            System(SearchBackward) => {
                self.view.enter_search();
                self.view.set_search_backward();
                self.set_prompt(PromptType::Search);
            }
            System(SearchWordForward) => self.search_word_under_cursor(true),
            System(SearchWordBackward) => self.search_word_under_cursor(false),
            System(CommandMode) => self.set_prompt(PromptType::CommandMode),
            System(Save) => self.handle_save_command(),
            Edit(edit_command) => self.view.handle_edit_command(edit_command),
            Move(move_command) => self.view.handle_move_command(move_command),
        }
    }
}

impl<R: Renderer, E: EventSource> Drop for Editor<R, E> {
    fn drop(&mut self) {
        let _ = self.renderer.terminate();
        if self.should_quit {
            eprintln!("Goodbye.");
        }
    }
}

/// 独立函数，避免 Rust 借引用检查器在方法调用时的字段互斥问题。
fn render_components(
    renderer: &mut dyn Renderer,
    command_bar: &mut CommandBar,
    message_bar: &mut MessageBar,
    status_bar: &mut StatusBar,
    view: &mut View,
    terminal_size: Size,
    in_prompt: bool,
) {
    let bottom_bar_row = terminal_size.height.saturating_sub(1);
    let _ = renderer.hide_caret();
    if in_prompt {
        command_bar.render(renderer, bottom_bar_row);
    } else {
        message_bar.render(renderer, bottom_bar_row);
    }
    if terminal_size.height > 1 {
        status_bar.render(renderer, terminal_size.height.saturating_sub(2));
    }
    if terminal_size.height > 2 {
        view.render(renderer, 0);
    }
}
