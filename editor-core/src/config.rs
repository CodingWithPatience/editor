use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 编辑器配置，支持从 TOML 文件加载。
///
/// 配置文件查找顺序：
/// 1. `EDITOR_CONFIG` 环境变量指定的路径
/// 2. 当前工作目录下 `.editor.toml`
/// 3. 用户主目录 `~/.editor.toml`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default)]
    pub font: FontConfig,
    #[serde(default)]
    pub cursor: CursorConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    /// 滚动边距（行数）：光标距可视区域边界小于此值时触发滚动，默认 5
    #[serde(default = "default_scrolloff")]
    pub scrolloff: usize,
}

fn default_scrolloff() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// 自定义字体路径，None 表示使用 OS 默认 CJK 字体
    pub path: Option<String>,
    /// 字体大小（像素）
    #[serde(default = "default_font_size")]
    pub size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorConfig {
    /// 光标闪烁间隔（毫秒）
    #[serde(default = "default_blink_interval_ms")]
    pub blink_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub background: Option<[u8; 3]>,
    pub text: Option<[u8; 3]>,
    pub cursor_color: Option<[u8; 3]>,
    pub selection_bg: Option<[u8; 3]>,
    pub search_match_bg: Option<[u8; 3]>,
    pub search_selected_bg: Option<[u8; 3]>,
    pub prompt_bg: Option<[u8; 3]>,
    pub line_number: Option<[u8; 3]>,
    pub line_number_active: Option<[u8; 3]>,
    pub keyword: Option<[u8; 3]>,
    pub string_color: Option<[u8; 3]>,
    pub comment: Option<[u8; 3]>,
    pub number: Option<[u8; 3]>,
    pub type_color: Option<[u8; 3]>,
    pub known_value: Option<[u8; 3]>,
    pub char_color: Option<[u8; 3]>,
    pub lifetime: Option<[u8; 3]>,
}

fn default_font_size() -> f32 {
    16.0
}

fn default_blink_interval_ms() -> u64 {
    500
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            cursor: CursorConfig::default(),
            theme: ThemeConfig::default(),
            scrolloff: default_scrolloff(),
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            path: None,
            size: default_font_size(),
        }
    }
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            blink_interval_ms: default_blink_interval_ms(),
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            background: None,
            text: None,
            cursor_color: None,
            selection_bg: None,
            search_match_bg: None,
            search_selected_bg: None,
            prompt_bg: None,
            line_number: None,
            line_number_active: None,
            keyword: None,
            string_color: None,
            comment: None,
            number: None,
            type_color: None,
            known_value: None,
            char_color: None,
            lifetime: None,
        }
    }
}

impl EditorConfig {
    /// 从配置文件加载配置。如果文件不存在或解析失败，返回默认配置。
    pub fn load() -> Self {
        if let Some(path) = Self::find_config_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str::<EditorConfig>(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// 按优先级查找配置文件路径。
    fn find_config_file() -> Option<PathBuf> {
        // 1. 环境变量
        if let Ok(path) = std::env::var("EDITOR_CONFIG") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
        // 2. 当前工作目录
        if let Ok(cwd) = std::env::current_dir() {
            let p = cwd.join(".editor.toml");
            if p.exists() {
                return Some(p);
            }
        }
        // 3. 用户主目录
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".editor.toml");
            if p.exists() {
                return Some(p);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = EditorConfig::default();
        assert_eq!(config.font.size, 16.0);
        assert!(config.font.path.is_none());
        assert_eq!(config.cursor.blink_interval_ms, 500);
        assert!(config.theme.background.is_none());
    }

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
[font]
path = "/usr/share/fonts/test.ttf"
size = 14.0

[cursor]
blink_interval_ms = 300

[theme]
background = [30, 30, 30]
text = [210, 210, 210]
keyword = [100, 149, 237]
"#;
        let config: EditorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font.path, Some("/usr/share/fonts/test.ttf".to_string()));
        assert_eq!(config.font.size, 14.0);
        assert_eq!(config.cursor.blink_interval_ms, 300);
        assert_eq!(config.theme.background, Some([30, 30, 30]));
        assert_eq!(config.theme.text, Some([210, 210, 210]));
        assert_eq!(config.theme.keyword, Some([100, 149, 237]));
    }

    #[test]
    fn parse_partial_config() {
        let toml_str = r#"
[font]
size = 20.0
"#;
        let config: EditorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font.size, 20.0);
        assert!(config.font.path.is_none());
        assert_eq!(config.cursor.blink_interval_ms, 500); // 默认值
    }

    #[test]
    fn parse_empty_config() {
        let config: EditorConfig = toml::from_str("").unwrap();
        assert_eq!(config.font.size, 16.0);
        assert_eq!(config.cursor.blink_interval_ms, 500);
    }

    #[test]
    fn parse_theme_only() {
        let toml_str = r#"
[theme]
background = [0, 0, 0]
"#;
        let config: EditorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.theme.background, Some([0, 0, 0]));
        assert!(config.theme.text.is_none());
    }

    #[test]
    fn serialize_default() {
        let config = EditorConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("size"));
        assert!(toml_str.contains("blink_interval_ms"));
    }

    #[test]
    fn roundtrip() {
        let config = EditorConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: EditorConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.font.size, config.font.size);
        assert_eq!(parsed.cursor.blink_interval_ms, config.cursor.blink_interval_ms);
    }
}
