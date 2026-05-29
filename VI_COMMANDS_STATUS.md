# Vi 编辑器命令实现状态 & Visual 模式规划

## 一、当前实现状态总览

### 已完成模块

| 模块 | 状态 |
|------|------|
| 三模式系统 (Normal/Insert/Command) | 完成 |
| NormalPending 多键序列 + 游标区分显示 | 完成 |
| hjkl 方向移动 | 完成 |
| w/W/e/E/b/B 词级移动 | 完成 |
| 0/^/$ 行级移动 | 完成 |
| gg/G 首/尾行跳转 | 完成 |
| PageUp/PageDown 翻页 | 完成 |
| Ctrl+D/Ctrl+U 半页滚动 | 完成 |
| i/I/a/A/o/O 进入 Insert | 完成 |
| s/S/C 删除并入 Insert | 完成 |
| x/X/D 删除字符/到行尾 | 完成 |
| dd / dw / d+移动 | 删除行/词/到目标（通用 operator+motion） | 完成 |
| cc / cw / c+移动 | 修改行/词/到目标（通用 operator+motion） | 完成 |
| yy / y+移动 | 复制行/到目标（通用 operator+motion） | 完成 |
| Y | Shift+Y = yy 复制行 | 完成 |
| p/P 粘贴 | 完成 |
| r+char 替换字符 | 完成 |
| u/Ctrl+R 撤销/重做 | 完成 |
| n/N 重复搜索 | 完成 |
| Ctrl+S/Ctrl+Q 保存/退出 | 完成 |
| Ctrl+F 搜索 | 完成 |
| Ex 命令 (:w/:q/:e/:wq) | 完成 |
| Ctrl+Delete/Ctrl+Backspace 删除词 | 完成 |
| f/F/t/T + char 行内字符搜索 | 完成 |
| ;/, 重复/反向重复 f/t 搜索 | 完成 |
| J 合并行 | 完成 |
| ~ 切换大小写 | 完成 |
| . 重复上次修改 | 完成 |
| << / >> 缩进/反缩进 | 完成 |
| / 从 Normal 前向搜索 | 完成 |
| ? 从 Normal 反向搜索 | 完成 |
| * / # 搜索光标下单词 | 完成 |
| % 括号匹配跳转 | 完成 |
| Ctrl+W / Ctrl+U 插入模式快捷删除 | 完成 |
| Ctrl+[ 替代 Esc | 完成 |
| :set nu / :set nonu 行号开关 | 完成 |
| :noh 清除搜索高亮 | 完成 |
| :x 保存退出 | 完成 |
| 数字计数前缀 (3j, 5dd 等) | 完成 |
| Visual 模式高亮 (v/V 进入, 移动选中, Esc 退出) | 完成 |
| Visual 模式 d/x/y/c 删除/复制/修改选中 | 完成 |
| Visual 模式 ~ 切换选中大小写 | 完成 |
| Visual 模式 p/P 替换选中文本 | 完成 |
| Visual 模式 >/< 多行缩进 | 完成 |
| Visual 模式 f/F/t/T 行内选中 + ;/, 重复 | 完成 |
| Visual 模式 o 交换锚点/光标 | 完成 |
| 光标行行号高亮 | 完成 |
| 状态栏选择计数 | 完成 |
| Rust 语法高亮 | 完成 |
