//! 按键映射模块。ComputeModeAction 和 FromKey trait 定义及实现在 editor-core 中，
//! 本模块仅重导出以便 editor-term 使用原有导入路径。

pub use editor_core::command::mode::ComputeModeAction;
