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
    let args: Vec<String> = std::env::args().collect();
    let gui_mode = args.iter().any(|a| a == "--gui");
    let filename = args.iter().find(|a| !a.starts_with("--")).cloned();

    if gui_mode {
        launch_gui(filename);
        return;
    }

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

/// 启动 GUI 版本。通过 spawn 同目录下的 editor-gui 二进制。
fn launch_gui(filename: Option<String>) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    // 尝试同目录下的 editor-gui（发布模式）
    let gui_names = if cfg!(windows) {
        vec!["editor-gui.exe", "editor-gui"]
    } else {
        vec!["editor-gui"]
    };

    if let Some(dir) = &exe_dir {
        for name in &gui_names {
            let gui_path = dir.join(name);
            if gui_path.exists() {
                let mut cmd = std::process::Command::new(&gui_path);
                if let Some(ref f) = filename {
                    cmd.arg(f);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                }
                match cmd.status() {
                    Ok(_) => return,
                    Err(e) => {
                        eprintln!("启动 GUI 失败: {e}");
                        return;
                    }
                }
            }
        }
    }

    // fallback: 尝试 cargo run（开发模式）
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(["run", "-p", "editor-gui", "--"]);
    if let Some(ref f) = filename {
        cmd.arg(f);
    }
    match cmd.status() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("无法启动 GUI 版本: {e}");
            eprintln!("请确认 editor-gui 已编译（cargo build -p editor-gui）");
        }
    }
}
