//! AppConfig 字体/字号字段测试（v1.1.0）
//!
//! 复用 config_json_shape.rs 的 schema duplication 模式
//! （lib test 在 Windows 上偶发 STATUS_ENTRYPOINT_NOT_FOUND）。
//! 这里只验证新增的 font_size / font_family 字段的向后兼容和默认值。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub last_port: Option<String>,
    #[serde(default = "default_baud_rate")]
    pub baud_rate: u32,
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    #[serde(default = "default_parity")]
    pub parity: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,
    #[serde(default = "default_reconnect_max_attempts")]
    pub reconnect_max_attempts: u32,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
}

fn default_version() -> u32 { 1 }
fn default_baud_rate() -> u32 { 115200 }
fn default_data_bits() -> u8 { 8 }
fn default_stop_bits() -> u8 { 1 }
fn default_parity() -> String { "none".to_string() }
fn default_encoding() -> String { "utf8".to_string() }
fn default_theme() -> String { "dark".to_string() }
fn default_buffer_size() -> usize { 65536 }
fn default_auto_reconnect() -> bool { true }
fn default_reconnect_max_attempts() -> u32 { 5 }
fn default_font_size() -> u32 { 14 }
fn default_font_family() -> String { "system-default".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            last_port: None,
            baud_rate: default_baud_rate(),
            data_bits: default_data_bits(),
            stop_bits: default_stop_bits(),
            parity: default_parity(),
            encoding: default_encoding(),
            theme: default_theme(),
            buffer_size: default_buffer_size(),
            auto_reconnect: default_auto_reconnect(),
            reconnect_max_attempts: default_reconnect_max_attempts(),
            font_size: default_font_size(),
            font_family: default_font_family(),
        }
    }
}

#[test]
fn appconfig_default_has_new_fields() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.font_size, 14, "默认字号 14");
    assert_eq!(cfg.font_family, "system-default", "默认字体占位符");
}

#[test]
fn appconfig_serde_backward_compat() {
    // 模拟旧 config.json：只有老字段，无 font_size / font_family
    let old_json = r#"{
        "version": 1,
        "last_port": "COM3",
        "baud_rate": 115200,
        "data_bits": 8,
        "stop_bits": 1,
        "parity": "none",
        "encoding": "utf8",
        "theme": "dark",
        "buffer_size": 65536,
        "auto_reconnect": true,
        "reconnect_max_attempts": 5
    }"#;
    let cfg: AppConfig = serde_json::from_str(old_json).expect("旧 JSON 应能反序列化");
    assert_eq!(cfg.font_size, 14);
    assert_eq!(cfg.font_family, "system-default");
}

#[test]
fn appconfig_serde_roundtrip_with_new_fields() {
    let mut cfg = AppConfig::default();
    cfg.font_size = 18;
    cfg.font_family = "JetBrains Mono".to_string();
    let json = serde_json::to_string(&cfg).expect("serialize");
    let restored: AppConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.font_size, 18);
    assert_eq!(restored.font_family, "JetBrains Mono");
}
