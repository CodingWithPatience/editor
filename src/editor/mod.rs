mod terminal;
mod document_status;
mod ui_components;
mod command;
mod line;

mod annotated_string;
mod annotation;
pub mod annotation_type;
mod file_type;

use crate::editor::command::mode::{CommandValue, Mode};
use crate::editor::command::Command::{self, Edit, Move, System};
use crate::editor::line::Line;
use crate::editor::terminal::{Terminal, OFFSET_COL};
use crate::editor::ui_components::command_bar::CommandBar;
use crate::editor::ui_components::message_bar::MessageBar;
use crate::editor::ui_components::status_bar::StatusBar;
use crate::editor::ui_components::ui_component::UIComponent;
use crate::prelude::{Position, Size};
use command::edit::Edit::InsertNewline;
use command::move_command::Move as MoveType;
use command::move_command::Move::{Down, Left, Right, Up};
use command::system::System::{CommandMode, Dismiss, Quit, Resize, Save, Search, SearchBackward, SearchWordForward, SearchWordBackward};
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::env;
use std::io::Error;
use std::panic::{set_hook, take_hook};
use ui_components::view::View;

pub const NAME: &str = env!("CARGO_PKG_NAME");

const QUIT_TIMES: u8 = 3;

#[derive(Default, Eq, PartialEq)]
enum PromptType {
    Save,
    Search,
    CommandMode,
    #[default]
    None
}

impl PromptType {

    fn is_none(&self) -> bool {
        *self == Self::None
    }

}

#[derive(Default)]
pub struct Editor {
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

impl Editor {

    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();
        set_hook(Box::new(move |info| {
            let _ = Terminal::terminate();
            current_hook(info);
        }));
        Terminal::initialize()?;

        let mut editor = Self::default();
        let size = Terminal::size().unwrap_or_default();
        editor.handle_resize_command(size);
        editor.update_message("HELP: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit");
        let args :Vec<String> = env::args().collect();
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
            match read() {
                Ok(event) => self.evaluate_event(event),
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event {err:?}");
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        let _ = err;
                    }
                }
            }
            self.refresh_status();
        }
    }

    /// Accumulate a count prefix digit in Normal mode. Returns true if consumed.
    fn try_accumulate_count(&mut self, event: &KeyEvent) -> bool {
        if self.mode == Mode::Normal {
            if let (KeyCode::Char(ch), KeyModifiers::NONE) = (event.code, event.modifiers) {
                if ch.is_ascii_digit() {
                    let digit = ch.to_digit(10).unwrap() as usize;
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

    fn evaluate_event(&mut self, event: Event) {
        let should_process = match &event {
            Event::Key(KeyEvent { kind, .. }) => kind == &KeyEventKind::Press,
            Event::Resize(_, _) => true,
            _ => false
        };
        if should_process {
            match event {
                Event::Key(key_event) => {
                    // Handle gg and f/F/t/T in Visual mode
                    if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                        // gg: two g keys → FirstLine, stay Visual
                        if let (KeyCode::Char('g'), KeyModifiers::NONE) = (key_event.code, key_event.modifiers) {
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
                        // f/F/t/T + char: first key sets pending kind, second key produces move
                        if let Some(fkind) = self.visual_pending_find {
                            if let (KeyCode::Char(ch), _modifiers) = (key_event.code, key_event.modifiers) {
                                let cmd = match fkind {
                                    'f' => MoveType::FindChar(ch),
                                    'F' => MoveType::FindCharBackward(ch),
                                    't' => MoveType::TillChar(ch),
                                    'T' => MoveType::TillCharBackward(ch),
                                    _ => { self.visual_pending_find = None; self.visual_pending_textobj = false; return; }
                                };
                                self.visual_pending_find = None;
                                let n = self.count_prefix.take().unwrap_or(1);
                                for _ in 0..n {
                                    self.view.handle_move_command(cmd);
                                }
                                self.visual_pending_textobj = false;
                                return;
                            }
                            self.visual_pending_find = None;
                            self.visual_pending_textobj = false;
                            return;
                        }
                        match (key_event.code, key_event.modifiers) {
                            (KeyCode::Char('f'), KeyModifiers::NONE) => { self.visual_pending_find = Some('f'); self.visual_pending_textobj = false; return; }
                            (KeyCode::Char('F'), KeyModifiers::SHIFT) => { self.visual_pending_find = Some('F'); self.visual_pending_textobj = false; return; }
                            (KeyCode::Char('t'), KeyModifiers::NONE) => { self.visual_pending_find = Some('t'); self.visual_pending_textobj = false; return; }
                            (KeyCode::Char('T'), KeyModifiers::SHIFT) => { self.visual_pending_find = Some('T'); self.visual_pending_textobj = false; return; }
                            // o: swap selection anchor and cursor
                            (KeyCode::Char('o'), KeyModifiers::NONE) => {
                                self.view.swap_selection_ends();
                                self.visual_pending_textobj = false;
                                return;
                            }
                            // i: enter text-object pending (viw = select inner word)
                            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                                self.visual_pending_textobj = true;
                                return;
                            }
                            // w after i (viw): select inner word
                            (KeyCode::Char('w'), KeyModifiers::NONE) if self.visual_pending_textobj => {
                                self.visual_pending_textobj = false;
                                self.view.select_inner_word();
                                return;
                            }
                            _ => {}
                        }
                        self.visual_pending_textobj = false;
                    }
                    // Accumulate count prefix in Normal mode
                    if self.try_accumulate_count(&key_event) {
                        return;
                    }
                    // Clear count on Esc (cancel any pending state)
                    if matches!((key_event.code, key_event.modifiers), (KeyCode::Esc, KeyModifiers::NONE)) {
                        self.count_prefix = None;
                        self.visual_pending_find = None;
                    }
                    let mode_action = self.mode.compute_mode_action(key_event);
                    if self.mode != mode_action.mode {
                        if mode_action.mode == Mode::Insert {
                            self.view.take_snapshot();
                        }
                        self.mode = mode_action.mode;
                        self.view.set_mode(self.mode)
                    }
                    if let Some(command_list) = mode_action.command_list {
                        let n = self.count_prefix.take().unwrap_or(1);
                        for _ in 0..n {
                            for &command in &command_list {
                                self.process_command(command);
                            }
                        }
                    }
                },
                Event::Resize(width_u16, height_u16) => {
                    let command = System(Resize(Size {
                        width: width_u16 as usize,
                        height: height_u16 as usize
                    }));
                    self.process_command(command);
                },
                _ => {
                    println!("Event not supported: {event:?}");
                }
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
            PromptType::None => self.process_command_no_prompt(command)
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
            self.update_message(&format!("WARNING! file has unsaved changes. Press Ctrl-Q {} more times to quit", QUIT_TIMES - self.quit_times - 1));
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
        let _ = Terminal::hide_caret();
        if self.in_prompt() {
            self.command_bar.render(bottom_bar_row);
        } else {
            self.message_bar.render(bottom_bar_row);
        }
        if self.terminal_size.height > 1 {
            self.status_bar.render(self.terminal_size.height.saturating_sub(2));
        }
        if self.terminal_size.height > 2 {
            self.view.render(0);
        }
        let new_caret_pos = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.caret_position_col()
            }
        } else {
            self.view.caret_position()
        };
        debug_assert!(new_caret_pos.col <= self.terminal_size.width);
        debug_assert!(new_caret_pos.row <= self.terminal_size.height);
        let _ = Terminal::move_caret_to(new_caret_pos);
        let _ = Terminal::set_cursor_style(self.mode.cursor_style());
        let _ = Terminal::show_caret();
        let _ = Terminal::execute();
    }

    fn refresh_status(&mut self) {
        let status = self.view.get_status();
        let title = format!("{} {NAME}", status.filename);
        self.status_bar.update_status(status);
        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
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
            height: size.height.saturating_sub(2)
        });
        let bar_size = Size {
            height: 1,
            width: size.width.saturating_sub(OFFSET_COL)
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
            // Save for n/N repeat
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
                self.command_bar.set_prompt("Search (Esc to cancel), Up or down arrows to navigate: ");
            }
            PromptType::CommandMode => {
                self.command_bar.set_prompt(":");
            },
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
            System(Save | Resize(_) | Search | SearchBackward | SearchWordForward | SearchWordBackward | Quit | CommandMode) | Move(_) => {}
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
            System(Save | Resize(_) | Search | SearchBackward | SearchWordForward | SearchWordBackward | Quit | CommandMode) | Move(_) => {}
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
                    Ok(command_value) => {
                        match command_value {
                            CommandValue::Quit | CommandValue::ForceQuit | CommandValue::SaveAndQuit | CommandValue::SaveAsAndQuit(_) => {
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
                        }
                    }
                    Err(e) => self.update_message(e.as_str()),
                }
                self.view.dismiss_command_mode();
            }
            Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
            }
            Move(Right) => self.command_bar.move_right(),
            Move(Left) => self.command_bar.move_left(),
            System(Save | Resize(_) | Search | SearchBackward | SearchWordForward | SearchWordBackward | Quit | CommandMode) | Move(_) => {}
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
            Move(move_command) => self.view.handle_move_command(move_command)
        }
    }
}

impl Drop for Editor {

    fn drop(&mut self) {
        let _ = Terminal::terminate();
        if self.should_quit {
            let _ = Terminal::print("Goodbye.\r\n");
        }
    }

}