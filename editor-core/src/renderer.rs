use crate::annotated_string::AnnotatedString;
use crate::prelude::{Position, RowIdx, Size};

/// 光标样式枚举，各前端映射为对应平台的光标样式。
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CursorStyle {
    Block,
    Bar,
    UnderScore,
    BlinkingBlock,
    BlinkingBar,
    BlinkingUnderScore,
}

/// 渲染后端 trait。终端和 GUI 前端各自实现，将编辑器状态渲染到对应平台。
pub trait Renderer {
    /// 初始化渲染环境（如进入 alternate screen、raw mode）
    fn initialize(&mut self) -> Result<(), String>;

    /// 清理并退出渲染环境
    fn terminate(&mut self) -> Result<(), String>;

    /// 获取渲染区域可用尺寸（宽度已减去行号区域）
    fn size(&self) -> Result<Size, String>;

    /// 清空整个屏幕
    fn clear_screen(&mut self) -> Result<(), String>;

    /// 在指定行渲染纯文本
    fn render_text_row(&mut self, row: RowIdx, text: &str) -> Result<(), String>;

    /// 在指定行渲染反转色文本（用于状态栏）
    fn render_inverted_row(&mut self, row: RowIdx, text: &str) -> Result<(), String>;

    /// 在指定行渲染带语法高亮的文本
    fn render_annotated_row(
        &mut self,
        row: RowIdx,
        line_idx: usize,
        annotated_string: &AnnotatedString,
    ) -> Result<(), String>;

    /// 设置是否显示行号
    fn set_show_line_numbers(&mut self, show: bool);

    /// 设置当前光标所在行（用于行号高亮）
    fn set_cursor_line(&mut self, line: usize);

    /// 移动光标到指定位置（屏幕坐标）
    fn move_caret_to(&mut self, position: Position) -> Result<(), String>;

    /// 设置光标样式
    fn set_cursor_style(&mut self, style: CursorStyle) -> Result<(), String>;

    /// 隐藏光标
    fn hide_caret(&mut self) -> Result<(), String>;

    /// 显示光标
    fn show_caret(&mut self) -> Result<(), String>;

    /// 设置窗口标题
    fn set_title(&mut self, title: &str) -> Result<(), String>;

    /// 将缓冲的命令刷新到输出
    fn flush(&mut self) -> Result<(), String>;
}
