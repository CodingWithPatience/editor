#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::print_stdout,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::integer_division
)]

mod crossterm_event_source;
mod crossterm_renderer;
mod editor;
mod keymap;

use crate::crossterm_event_source::CrosstermEventSource;
use crate::crossterm_renderer::CrosstermRenderer;
use crate::editor::Editor;
use editor_core::renderer::Renderer;
use std::panic::{set_hook, take_hook};

fn main() {
    let renderer = CrosstermRenderer::default();
    let event_source = CrosstermEventSource;
    // 保存当前 panic hook，在退出前恢复终端状态
    let current_hook = take_hook();
    set_hook(Box::new(move |info| {
        let mut r = CrosstermRenderer::default();
        let _ = r.terminate();
        current_hook(info);
    }));
    let mut editor = Editor::new(renderer, event_source).unwrap();
    editor.run();
}
