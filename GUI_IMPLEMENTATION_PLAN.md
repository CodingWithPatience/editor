# GUI 编辑器迁移实现计划

> **活文档**：每完成一个阶段后更新状态，记录已完成模块，调整余下阶段优先级。

---

## 当前状态

- **已完成阶段**：阶段 1, 阶段 2, 阶段 3, 阶段 4, 阶段 5
- **当前进行中**：阶段 6 - 双模式共存 + 完善
- **最后更新**：2026-05-28

---

## 已完成：阶段 1 - Workspace 基础设施 + 核心提取

### 完成摘要

将单一 crate 项目重构为 Cargo workspace 三层架构的初始形态：

```
editor/
├── Cargo.toml                   # [workspace] 清单
├── editor-core/                 # 平台无关核心 (lib)
│   └── src/
│       ├── prelude/             # Position, Size, Location, GraphemeType, 类型别名
│       ├── line/                # Line, TextFragment, GraphemeWidth
│       ├── annotated_string/    # AnnotatedString 及迭代器
│       ├── annotation.rs / annotation_type.rs
│       ├── document_status.rs / file_type.rs
│       ├── buffer.rs / file_info.rs / search_info.rs / search_direction.rs
│       ├── highlighter/         # Highlighter + SyntaxHighlighter + Rust/Search 高亮
│       └── command/             # Command/Edit/Move/System/Mode/PendingKey/CommandValue 枚举
├── editor-term/                 # 终端前端 (bin)
│   └── src/
│       ├── main.rs              # 入口
│       ├── keymap.rs            # crossterm KeyEvent → Command 映射 + compute_mode_action
│       ├── mode_ext.rs          # Mode::cursor_style() 扩展 (crossterm)
│       └── editor/
│           ├── mod.rs           # Editor 结构体 (事件循环)
│           ├── terminal/        # Terminal 静态类 (crossterm 渲染)
│           └── ui_components/   # View, StatusBar, MessageBar, CommandBar
└── GUI_IMPLEMENTATION_PLAN.md   # 本计划文档
```

### 关键技术决策
- **命令枚举剥离**：Editor/Edit/Move/System/Mode 等枚举类型移入 editor-core，crossterm 相关的 `TryFrom<KeyEvent>` 实现和 `compute_mode_action` 函数移入 editor-term/src/keymap.rs
- **孤儿规则解决**：使用自定义 `FromKeyEvent` trait 替代 `TryFrom<KeyEvent>`（避免孤儿规则冲突）
- **光标样式解耦**：`Mode::cursor_style()` 通过 `ModeExt` trait 扩展到 editor-term
- **View 保持原位**：View 结构体（含 draw() 方法）暂时留在 editor-term，后续阶段再提取核心逻辑到 EditorCore

### 验证结果
- `cargo build --workspace` 零错误零警告
- `cargo run -p editor-term test.txt` 正常启动，语法高亮、行号、状态栏全部正常

---

## 架构总览

```
editor/                          # workspace 根
├── Cargo.toml                   # [workspace] 清单
├── editor-core/                 # 平台无关核心 (lib crate)
│   └── src/                     # Buffer, Line, Command, Syntax, EditorCore
├── editor-term/                 # 终端前端 (bin crate)
│   └── src/                     # CrosstermRenderer, 事件循环, main.rs
└── editor-gui/                  # GUI 前端 (bin crate, 阶段3新增)
    └── src/                     # eframe::App, egui 渲染, main.rs
```

核心抽象：
- **EditorCore**：平台无关的编辑器状态机，暴露 `process_key_event()`, `get_visible_lines()`, `cursor_position()`, `get_status()` 等
- **trait Renderer**：渲染后端抽象（crossterm 实现 / egui 实现）
- **trait EventSource**：事件源抽象（crossterm 阻塞读取 / egui 帧事件）
- **统一 Key enum**：两个前端各自转换到统一按键类型

---

## 阶段 1：Workspace 基础设施 + 核心提取 ✅

> **优先级：P0** | **状态：已完成**

### 1.1 创建 workspace 结构
- [ ] 根 `Cargo.toml` 改为 workspace 清单
- [ ] 创建 `editor-core/Cargo.toml`
- [ ] 创建 `editor-term/Cargo.toml`

### 1.2 移动平台无关代码到 editor-core
- [ ] prelude/ — Position, Size, Location, GraphemeType, 类型别名
- [ ] line/ — Line, TextFragment, GraphemeWidth
- [ ] annotated_string/ — AnnotatedString 及迭代器
- [ ] annotation.rs / annotation_type.rs
- [ ] document_status.rs / file_type.rs
- [ ] buffer.rs / file_info.rs / search_info.rs / search_direction.rs
- [ ] highlighter/ — Highlighter + SyntaxHighlighter + Rust/搜索高亮
- [ ] command/ — Command/Edit/Move/System/Mode 枚举定义

### 1.3 创建 EditorCore 结构体
- [ ] `editor-core/src/core.rs` — EditorCore 状态机
- [ ] 核心方法：new, load, process_command, get_visible_lines, cursor_position, get_status, mode, handle_key_event
- [ ] CoreEffect 枚举，BufferSnapshot, FindKind

### 1.4 创建 editor-term 前端适配
- [ ] `editor-term/src/main.rs` — 薄入口
- [ ] `editor-term/src/keymap.rs` — crossterm KeyEvent -> 统一 Key
- [ ] terminal/ 模块迁移
- [ ] ui/ 组件迁移

### 1.5 验证
- [ ] `cargo build --workspace` 通过
- [ ] `cargo run -p editor-term` 功能一致

---

---

## 已完成：阶段 2 - 核心抽象层

### 完成摘要

定义了三个核心抽象，将终端/GUI 前端完全解耦：

**新增 editor-core 模块：**
- `key.rs` — 统一 `Key` 枚举（Char/Shift/Ctrl/方向键/功能键/CtrlBackspace/CtrlDelete）
- `event_source.rs` — `EventSource` trait + `InputEvent` 枚举（Key/Resize）
- `renderer.rs` — `Renderer` trait + `CursorStyle` 枚举（10 个渲染方法）
- `prelude/mod.rs` — 添加 `GUTTER_WIDTH = 6` 常量

**新增 editor-term 模块：**
- `crossterm_renderer.rs` — `CrosstermRenderer` 实现 `Renderer` trait，将原来 `Terminal` 静态类 + 全局原子的逻辑转为实例方法
- `crossterm_event_source.rs` — `CrosstermEventSource` 实现 `EventSource` trait，包装 `crossterm::event::read()`

**关键重构：**
- `keymap.rs` — 完全重写，`compute_mode_action` 现在接收统一 `Key` 而非 `crossterm::KeyEvent`；`FromKeyEvent` → `FromKey` trait
- `editor/mod.rs` — `Editor<R, E>` 泛型化，通过 `Renderer` trait 渲染、`EventSource` trait 读取事件
- `UIComponent` trait — `draw()` 方法改为接收 `&mut dyn Renderer`
- `View` — `draw()` 直接调用 `renderer.render_annotated_row()`；新增 `show_line_numbers` 字段
- `Mode::cursor_style()` — 移入 editor-core，返回 `CursorStyle` 而非 crossterm 类型
- 删除 `mode_ext.rs`（不再需要）
- 清理旧 `Terminal` 静态类所有死代码（仅保留 `attribute` 颜色映射模块）

### 消除的全局状态
- `SHOW_NUMBERS: AtomicBool` → `CrosstermRenderer.show_numbers: bool`
- `CURSOR_LINE: AtomicUsize` → `CrosstermRenderer.cursor_line: usize`
- `OFFSET_COL` → `editor_core::prelude::GUTTER_WIDTH`

### 验证结果
- `cargo build --workspace` 零错误零警告
- `cargo run -p editor-term test.txt` 语法高亮、行号、状态栏全部正常

---

## 阶段 2：核心抽象层（Renderer + EventSource trait） ✅

> **优先级：P0** | **状态：已完成**

### 核心任务
- [ ] 定义统一 Key enum（`editor-core/src/key.rs`）
- [ ] 定义 EventSource trait（`editor-core/src/event_source.rs`）
- [ ] 定义 Renderer trait（`editor-core/src/renderer.rs`）
- [ ] 实现 CrosstermRenderer（消除全局原子变量）
- [ ] 实现 CrosstermEventSource
- [ ] 重构 UI 组件使用 Renderer trait

---

## 已完成：阶段 3 - 最小 GUI 窗口（只读展示）

### 完成摘要

创建了 `editor-gui` crate，基于 egui 0.31 + eframe 0.31（当前 Rust 1.91.1 支持的最高兼容版本），实现了带语法高亮的只读文件展示窗口。

**新增 editor-gui 模块：**
- `editor-gui/Cargo.toml` — 依赖 editor-core + egui 0.31 + eframe 0.31
- `editor-gui/src/main.rs` — eframe 启动入口，解析命令行参数加载文件
- `editor-gui/src/app.rs` — `EditorApp` 结构体实现 `eframe::App` trait，持有 Buffer 实例
- `editor-gui/src/gui_renderer.rs` — `AnnotationType → egui::Color32` 颜色映射 + `LayoutJob` 构建函数

**渲染架构：**
- `EditorApp::build_highlighted_job()` — 遍历 Buffer 所有行，使用 Highlighter 获取语法高亮数据
- `gui_renderer::build_layout_job()` — 将每行 `AnnotatedString` 转换为带颜色的 `egui::text::LayoutJob`
- 行号区域（gutter）嵌入在 LayoutJob 中，每行前缀显示右对齐行号
- 暗色背景（`EDITOR_BG_COLOR: RGB(30,30,30)`）匹配编辑器风格
- 等宽字体（`egui::TextStyle::Monospace`）强制覆盖默认文本样式
- 使用 `egui::ScrollArea` 包裹 `Label` 实现滚动显示

**颜色映射：**
- 复用了终端版的 12 种 AnnotationType 颜色映射到等效的 egui Color32
- 行号使用与终端版一致的深灰色（非光标行）和蓝灰色（光标行）

### 验证结果
- `cargo build --workspace` 零错误零警告
- `cargo run -p editor-gui test.txt` 窗口正常启动

---

## 阶段 3：最小 GUI 窗口（只读展示） ✅

> **优先级：P1** | **状态：已完成**

### 核心任务
- [x] 创建 `editor-gui/` crate（egui + eframe）
- [x] 实现 `eframe::App`
- [x] AnnotationType -> egui::Color32 颜色映射
- [x] 带语法高亮的只读文本展示
- [x] 文件加载

---

## 已完成：阶段 4 - GUI 编辑功能

### 完成摘要

在 GUI 中实现完整的 vi 模式编辑系统，与终端版共享 `ComputeModeAction`/`FromKey` trait。

**关键架构变更：**
- `ComputeModeAction` + `FromKey` trait 从 `editor-term` 移入 `editor-core`（~250 行），实现真正的平台无关复用
- `editor-term/src/keymap.rs` 从 575 行简化为 4 行重导出

**新增 editor-gui 模块：**
- `keymap.rs` — `egui_to_key(egui::Key, Modifiers) → Option<Key>` 映射（140 行），显式处理所有字母/数字/符号键 + Ctrl/Shift 组合
- `app.rs` 重写（~410 行）：
  - **编辑状态机**：Mode, text_location, yank_register, undo_stack/redo_stack, count_prefix
  - **handle_edit()** — 处理 16 种 Edit 命令：Insert/InsertNewline/Delete/DeleteBackward/DeleteLine/DeleteToEnd/DeleteToNextWordStart/DeleteWholeLine/DeleteToLineStart/DeleteWord/DeleteWordBackward/YankLine/Paste/PasteAbove/Undo/Redo/JoinLines
  - **handle_move()** — 处理 20+ 种 Move 命令：hjkl/w/b/e/0/^/$/gg/G/w/W/e/E/b/B/PageUp/PageDown/HalfPage
  - **paste()/undo()/redo()** — 完整剪贴板 + 撤销系统（BufferSnapshot 机制）
  - **模式指示器**：底部状态栏显示 NORMAL（灰色）/ INSERT（绿色）/ VISUAL（蓝色）
  - **Insert 撤销点**：模式切换到 Insert 时自动 take_snapshot
  - **dd 自动 yank**：DeleteLine 将删除内容存入寄存器

### Code Review 修复项（6 个严重 + 4 个警告）
| 问题 | 修复 |
|------|------|
| 🔴 YankLine 多余 `\n` | 去掉 `format!("{content}\n")` → 直接存原始内容 |
| 🔴 DeleteToNextWordStart = d$ | 使用 `find_next_word_start_on_line()` 正确查找词边界 |
| 🔴 DeleteLine 不 yank | 捕获 `delete_line()` 返回值写入 yank_register |
| 🔴 Insert 模式无撤销 | Normal→Insert 切换时调用 take_snapshot |
| 🔴 DeleteWordBackward 静默忽略 | 添加 DeleteWord/DeleteWordBackward 处理分支 |
| 🟡 字母键用 format!/unwrap | 改为 26 个显式 match 分支 |
| 🟡 缺少数字/符号键 | 添加 Num0-9、[-=、[]、\\、' 等符号键映射 |
| 🟡 粘贴 clone 不必要 | `yank_register.clone()` → `yank_register` 借用 |

### Bug 修复（2026-05-27）
| 问题 | 严重程度 | 修复 |
|------|----------|------|
| Visual 模式下 selection 信息被提前清除 | 🔴 严重 | 将 `selection_anchor = None` 移到命令执行之后，确保 DeleteSelection/YankSelection 等命令能正确获取选区 |
| 滚动同步运算符优先级错误 | 🔴 严重 | `c_top < scroll_y \|\| c_bot > scroll_y + panel_h && panel_h > 0.0` → `(c_top < scroll_y \|\| c_bot > scroll_y + panel_h) && panel_h > 0.0` |
| yank_selection 使用 chars().count() | 🔴 严重 | 改用 `grapheme_to_byte()` 将 grapheme 索引转换为 byte 索引，正确处理 Unicode 字符 |
| 键盘事件处理后未立即重绘 | 🟡 中等 | 在 `process_key_events` 中调用 `ctx.request_repaint()` |
| as 强制类型转换违反规范 | 🟡 中等 | `(ch as u8 - b'0') as usize` → `ch.to_digit(10).unwrap_or(0) as usize` |
| Deindent 使用 chars() 而非 graphemes | 🟡 中等 | 改用 `line.graphemes(true).take_while(\|g\| *g == " ").count()` |
| YankSelection 返回 false | 🟡 中等 | 改为返回 `true` 以触发重绘 |
| 未使用的 needs_redraw 变量 | 🔵 清理 | 删除变量及所有相关赋值 |

### Bug 修复（2026-05-27 第二次）
| 问题 | 严重程度 | 修复 |
|------|----------|------|
| editor-term c+e/d+e 单词末尾字符未删除 | 🔴 严重 | 在 `delete_range_to_motion`、`change_range_to_motion`、`extract_text_range` 中对 EndOfWord 命令的 end 位置 +1 |
| editor-gui c+e/d+e 未生效 | 🔴 严重 | 在 GUI 版 `handle_edit` 中添加 `DeleteRangeToMove` 和 `ChangeRangeToMove` 命令处理 |
| editor-gui v+j/v+k 光标行未选中 | 🔴 严重 | `selection_range` 中对 `grapheme_idx + 1` 做 `.min(gc)` 限制，防止超出该行 grapheme 数量 |
| GUI 版 DeleteRangeToMove 对所有 motion 无条件 +1 | 🔴 严重 | 添加 motion 类型判断，仅对 EndOfWord 命令执行 +1 |
| GUI 版 delete_range 不支持反向删除 | 🔴 严重 | 在入口处规范化参数，确保 start <= end |
| selection_range 使用普通加法 | 🟡 中等 | 改为 `saturating_add(1)` 保持防御性编程一致性 |

### 验证结果
- `cargo build --workspace` 零错误零警告
- 终端版正常启动

---

## 阶段 4：GUI 编辑功能 ✅

> **优先级：P1** | **状态：已完成**

---

## 已完成：阶段 5 - GUI 完整功能

### 完成摘要

在 GUI 中实现搜索、命令模式、消息栏、保存/退出、文件对话框等功能。

**新增功能：**
- **搜索**：`/`/`?` 增量搜索、`n`/`N` 跳转、`*`/`#` 搜索光标下单词、Escape 取消恢复光标位置、搜索结果高亮（金色背景）
- **命令模式**：`:` 进入命令模式，支持 `:w`/`:q`/`:wq`/`:q!`/`:e <file>`/`:noh`/`:set nu` 等
- **消息栏**：5 秒自动过期的消息提示，位于状态栏上方
- **提示栏**：使用 egui TextEdit 实现，替代终端版的手动 CommandBar
- **保存/退出**：Ctrl-S 保存（无文件名时弹出文件对话框）、Ctrl-Q 退出（quit_times 3 次确认）
- **文件对话框**：rfd 原生文件对话框，`:e` 打开文件、`:w` 无文件名时保存

**架构变更：**
- 新增 `PromptType` 枚举管理提示栏状态
- `process_command()` 从 `_ => false` 改为完整的 `handle_system()` 分支
- 底部面板拆分为状态栏 + 提示/消息栏两层
- `process_key_events()` 在 prompt 活跃时跳过（由 TextEdit 处理输入）
- Highlighter 集成搜索查询和选中匹配位置

**Code Review 修复项（2 个严重 + 5 个警告）**
| 问题 | 严重程度 | 修复 |
|------|----------|------|
| quit_times 保存后未重置 | 🔴 严重 | 所有 save/save_as 成功后重置 quit_times = 0 |
| SaveAndQuit/SaveAsAndQuit 吞掉 I/O 错误 | 🔴 严重 | 改为 match 处理，错误时显示消息不退出 |
| search_next/prev 不必要 clone | 🟡 警告 | 删除 .clone()，直接借用 search_query |
| PromptType 缺少 Debug | 🟡 警告 | 添加 #[derive(Debug)] |
| new() 缺少文档注释 | 🟡 警告 | 添加 /// 注释 |
| search_word_under_cursor 不必要 clone | 🟡 警告 | 将 word 移入 search_query 后借用 |
| 行号布局调用结果未使用 | 🟡 警告 | 删除无效的 ui.fonts 调用 |

### 验证结果
- `cargo build --workspace` 零错误零警告

---

## 阶段 5：GUI 完整功能 ✅

> **优先级：P2** | **状态：已完成**

---

## 阶段 6：双模式共存 + 完善

> **优先级：P3** | **状态：未开始**

### 核心任务
- [ ] 统一入口（--gui 参数）
- [ ] 视觉优化（主题、字体、光标闪烁）
- [ ] 配置文件支持
- [ ] 单元测试 + 回归测试

---

## 风险清单

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| View 提取破坏编辑行为 | 高 | 先整体移动再逐步解耦 draw() |
| Renderer trait 与 egui 不匹配 | 中 | 阶段 3 后回调整理 trait 设计 |
| egui 大文件性能 | 中 | 提前测试 10000+ 行文件 |

---

## 验证方法（每阶段执行）

1. `cargo build --workspace` 无错误
2. `cargo clippy --workspace` 无关键警告
3. 终端版功能回归抽查
4. GUI 版（阶段 3+）目标功能验证
5. Code Review（根据 CLAUDE.md 规则）
