# GUI 编辑器迁移建议

## 一、推荐 GUI 库

### 首选: egui

**理由**:
- **即时模式渲染**与当前编辑器事件循环 (`Editor::run()` → `refresh_screen()` → redraw) 完美匹配，迁移改造成本最低
- `egui::TextEdit` 提供成熟的代码编辑控件，支持多行编辑、选区、光标样式
- 纯 Rust 实现，跨平台（含 WASM），无需系统 WebView
- 活跃维护，社区生态好，文档丰富
- 可以逐步迁移 — terminal 和 GUI 模式通过 trait 共享同一套 backend

**引入方式**:
```toml
[dependencies]
egui = "0.31"
eframe = "0.31"
```

### 备选方案

| 库 | 优点 | 缺点 |
|----|------|------|
| **iced** | Elm 架构，保留模式 UI，原生控件 | text editing widget 不够成熟，布局复杂 |
| **tauri** | 可用 Monaco/CodeMirror，成熟 | 体积大 (+30MB)，双进程架构，不纯 Rust |
| **floem** | GPU 加速，现代响应式 | 太新，API 不稳定 |
| **Slint** | 声明式 UI，嵌入式友好 | 非纯 Rust (.slint 文件)，商业许可证 |

---

## 二、架构重构方案

### 当前架构问题

当前单 crate 中 terminal 渲染 (crossterm)、buffer 管理、命令处理和 UI 组件耦合在一起：

```
src/editor/
├── terminal/        # crossterm 终端 I/O（平台相关）
├── ui_components/   # 渲染 + Buffer + View（混合）
├── command/         # Command/Mode 模型（平台无关）
├── line/            # Line 数据结构（平台无关）
└── annotated_string/ # 标注字符串（平台无关）
```

### 重构目标: Workspace 三层架构

```
editor/
├── Cargo.toml (workspace)
├── editor-core/         # 平台无关后端
│   ├── Cargo.toml       # 无 crossterm 依赖
│   └── src/
│       ├── buffer.rs    # Buffer, Line, Location
│       ├── command/     # Command, Mode, ModeAction, Edit, Move
│       ├── syntax/      # Rust 语法高亮
│       ├── prelude/     # 共享类型 (Position, Size, Location)
│       └── core.rs      # EditorCore: process_key_event(), get_state()
├── editor-term/         # 终端前端 (现有代码迁移)
│   ├── Cargo.toml       # 依赖 editor-core + crossterm
│   └── src/
│       ├── terminal/    # crossterm 渲染
│       ├── ui/          # status_bar, command_bar, message_bar
│       ├── view.rs      # 终端 View 渲染
│       └── main.rs      # 终端入口
└── editor-gui/          # GUI 前端 (新增)
    ├── Cargo.toml       # 依赖 editor-core + egui/eframe
    └── src/
        ├── app.rs       # eframe::App 实现
        ├── gui_view.rs  # egui 渲染 Buffer
        ├── components/  # 状态栏、命令栏控件
        └── main.rs      # GUI 入口
```

### 核心抽象: EditorCore

```rust
// editor-core/src/core.rs

pub struct EditorCore {
    mode: Mode,
    buffer: Buffer,
    text_location: Location,
    scroll_offset: Position,
    view_size: Size,
    undo_stack: Vec<BufferSnapshot>,
    redo_stack: Vec<BufferSnapshot>,
    yank_register: Option<String>,
    last_search_query: Option<Line>,
    status: DocumentStatus,
}

pub enum CoreEffect {
    Redraw,
    ShowMessage(String),
    StartSearch,
    StartCommandMode,
    StartSavePrompt,
    Quit,
}

impl EditorCore {
    /// 处理一次按键，返回前端需要的副作用列表
    pub fn process_key_event(&mut self, key: Key) -> Vec<CoreEffect>;

    /// 获取当前可见行的渲染数据
    pub fn get_visible_lines(&self) -> Vec<AnnotatedLine>;

    /// 获取光标位置 (行/列)
    pub fn cursor_position(&self) -> Position;

    /// 获取状态栏信息
    pub fn get_status(&self) -> DocumentStatus;

    /// 获取当前模式
    pub fn mode(&self) -> Mode;
}
```

### 前端 Trait

```rust
pub trait EditorFrontend {
    /// 渲染一帧 (由平台调用)
    fn render(&mut self, core: &EditorCore);
    
    /// 获取用户输入事件
    fn next_event(&mut self) -> FrontendEvent;
    
    /// 显示消息
    fn show_message(&mut self, msg: &str);
    
    /// 进入输入模式 (搜索/命令/保存)
    fn start_prompt(&mut self, prompt_type: PromptType);
}

pub enum FrontendEvent {
    Key(Key),
    Resize(Size),
    Quit,
}
```

---

## 三、关键实现思路

### 1. 事件循环统一

当前 terminal 的事件循环:
```rust
// editor-term: 已有模式
loop {
    self.refresh_screen();
    if self.should_quit { break; }
    match read() {                     // crossterm 阻塞读取
        Ok(event) => self.evaluate_event(event),
        Err(err) => { ... }
    }
}
```

GUI 的事件循环:
```rust
// editor-gui: egui/eframe 模式
impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // egui 自动处理事件循环，每帧调用 update()
        self.process_pending_events();  // 处理积压的键盘事件
        self.render_editor(ctx);        // 渲染编辑器 UI
        ctx.request_repaint();          // 请求下一帧
    }
}
```

### 2. 渲染适配

| 当前 (crossterm) | 将来 (egui) |
|---|---|
| `Terminal::print_annotated_row()` | `egui::TextEdit` 配合 `RichText` 段落 |
| `Terminal::print_inverted_row()` | `egui::Label` 或 `Frame` 设置 fill |
| `Terminal::move_caret_to()` | `TextEdit` 内置 cursor position |
| `Terminal::set_cursor_style()` | `TextEdit::cursor_at_range()` |
| `AnnotatedString` → 带颜色的行 | `RichText` 的 `LayoutJob` |

egui 代码编辑核心示例:
```rust
fn render_buffer(ui: &mut egui::Ui, core: &EditorCore) {
    let mut text = core.buffer_text();
    let mut layouter = |ui: &egui::Ui, string: &str, _wrap_width: f32| {
        // 构建带语法高亮的 LayoutJob
        let mut job = egui::text::LayoutJob::default();
        for line in string.lines() {
            for part in core.highlight_line(line) {
                job.append(
                    part.text, 0.0,
                    egui::TextFormat {
                        color: annotation_to_color(part.annotation_type),
                        ..Default::default()
                    }
                );
            }
            job.append("\n", 0.0, egui::TextFormat::default());
        }
        ui.fonts(|f| f.layout_job(job))
    };
    
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .layouter(&mut layouter)
            .desired_width(f32::INFINITY)
    );
}
```

### 3. 键盘输入映射

crossterm 的 `KeyEvent` 与 egui 的 `Key` 需要建立双向映射:

| 功能 | crossterm | egui/winit |
|------|-----------|------------|
| 方向键 | `KeyCode::Up/Down/Left/Right` | `egui::Key::ArrowUp/Down/Left/Right` |
| 字符 | `KeyCode::Char('h')` | `egui::Key::H` |
| 控制键 | `KeyModifiers::CONTROL` | `egui::Modifiers::ctrl` |
| Esc | `KeyCode::Esc` | `egui::Key::Escape` |
| Enter | `KeyCode::Enter` | `egui::Key::Enter` |

建议在 `editor-core` 中定义统一的 `Key` enum，两个前端各自转换:
```rust
// editor-core
pub enum Key {
    Char(char),
    Up, Down, Left, Right,
    PageUp, PageDown,
    Home, End,
    Enter, Escape, Tab, Backspace, Delete,
    Ctrl(char),   // 简化控制键表示
    Shift(char),  // 简化 Shift 表示
}
```

### 4. 语法高亮复用

当前 `Highlighter` 返回 `Vec<Annotation>` (byte-range + type)，这是平台无关的数据。GUI 端只需:

```rust
fn annotations_to_rich_text(
    line: &str,
    annotations: &[Annotation]
) -> egui::text::LayoutJob {
    let mut job = LayoutJob::default();
    for part in annotated_string_to_parts(line, annotations) {
        let color = annotation_type_to_egui_color(part.annotation_type);
        job.append(part.text, 0.0, egui::TextFormat { color, ..default() });
    }
    job
}
```

---

## 四、迁移步骤建议

### 阶段 1: 提取 editor-core (不改功能)

1. 创建 `editor-core/` crate
2. 将 `buffer.rs`, `line/`, `command/`, `annotated_string/`, `annotation.rs`, `annotation_type.rs`, `file_type.rs`, `document_status.rs`, `prelude/` 移入
3. 将 `View` 中平台无关的状态和逻辑提取为 `EditorCore`
4. `editor-term` 依赖 `editor-core`，保持现有功能 100% 不变

### 阶段 2: 最小 GUI 只读窗口

1. 创建 `editor-gui/` crate，依赖 `editor-core` + `egui`
2. 实现 `eframe::App`，显示一个带语法高亮的只读文本窗口
3. 验证 `editor-core` 的 `Buffer` → `get_visible_lines()` 接口正确

### 阶段 3: GUI 编辑功能

1. 添加键盘事件处理 → `EditorCore::process_key_event()`
2. 实现 Normal/Insert 模式切换
3. 实现基本编辑 (插入、删除、移动)
4. 添加状态栏和命令栏 widget

### 阶段 4: GUI 完整功能

1. 搜索功能 + 高亮
2. 命令模式 (`:` 提示)
3. 撤销/重做
4. 文件打开/保存对话框

### 阶段 5: 双模式共存

```rust
// 通过命令行参数或 feature flag 切换
fn main() {
    if std::env::args().any(|a| a == "--gui") {
        editor_gui::run();
    } else {
        editor_term::run();
    }
}
```

---

## 五、时机与风险

| 风险 | 缓解措施 |
|------|----------|
| editor-core API 设计不合理 | 先完整实现 terminal 功能，稳定后再提取 |
| egui 文本编辑性能不足 | 大文件可采用虚拟滚动 + 增量渲染 |
| 双模式代码重复 | 通过 CoreEffect 充分解耦，前端只负责渲染 |
| 高亮系统不兼容 | Annotation 已是平台无关的，只需转换颜色 |
