use crate::key::Key;
use crate::prelude::Size;

/// 平台无关的输入事件。
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// 按键事件
    Key(Key),
    /// 窗口/终端尺寸变更
    Resize(Size),
}

/// 事件源 trait，各前端实现各自的事件读取方式。
/// 终端实现为阻塞读取，GUI 实现为帧轮询。
pub trait EventSource {
    /// 获取下一个输入事件，无事件时返回 None。
    fn next_event(&mut self) -> Option<InputEvent>;
}
