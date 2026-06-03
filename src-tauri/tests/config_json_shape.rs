//! 配置文件 JSON schema 测试
//!
//! 不依赖 oh-my-serial lib（避免 Tauri 拉入 Windows 入口点问题），
//! 直接复制 schema 在这里做序列化兼容性验证，保证配置文件格式稳定。

use serde::{Deserialize, Serialize};

const CONFIG_VERSION: u32 = 1;

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
}

fn default_version() -> u32 { CONFIG_VERSION }
fn default_baud_rate() -> u32 { 115200 }
fn default_data_bits() -> u8 { 8 }
fn default_stop_bits() -> u8 { 1 }
fn default_parity() -> String { "none".to_string() }
fn default_encoding() -> String { "utf8".to_string() }
fn default_theme() -> String { "dark".to_string() }
fn default_buffer_size() -> usize { 65536 }
fn default_auto_reconnect() -> bool { true }
fn default_reconnect_max_attempts() -> u32 { 5 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
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
        }
    }
}

#[test]
fn default_values() {
    let c = AppConfig::default();
    assert_eq!(c.version, 1);
    assert_eq!(c.baud_rate, 115200);
    assert_eq!(c.data_bits, 8);
    assert_eq!(c.theme, "dark");
    assert!(c.auto_reconnect);
    assert_eq!(c.reconnect_max_attempts, 5);
    assert!(c.last_port.is_none());
}

#[test]
fn serialize_roundtrip() {
    let c = AppConfig {
        last_port: Some("COM5".into()),
        baud_rate: 921600,
        theme: "light".into(),
        auto_reconnect: false,
        ..Default::default()
    };
    let json = serde_json::to_string(&c).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(c, back);
}

#[test]
fn missing_fields_use_defaults() {
    // 模拟旧版本配置：只有 version + last_port
    let json = r#"{"version":1,"last_port":"COM3"}"#;
    let c: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(c.last_port.as_deref(), Some("COM3"));
    assert_eq!(c.baud_rate, 115200);
    assert_eq!(c.theme, "dark");
    assert!(c.auto_reconnect);
}

#[test]
fn empty_object_uses_all_defaults() {
    let c: AppConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(c.version, 1);
    assert_eq!(c.baud_rate, 115200);
    assert!(c.last_port.is_none());
}

#[test]
fn pretty_json_is_human_readable() {
    let c = AppConfig::default();
    let json = serde_json::to_string_pretty(&c).unwrap();
    // 人类可读：含换行 + 字段名
    assert!(json.contains("baud_rate"));
    assert!(json.contains("last_port"));
    assert!(json.lines().count() > 5);
}

#[test]
fn roundtrip_via_disk() {
    let dir = std::env::temp_dir().join(format!(
        "oh-my-serial-cfg-shapetest-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");

    let original = AppConfig {
        last_port: Some("COM7".into()),
        baud_rate: 460800,
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&original).unwrap();
    std::fs::write(&path, &json).unwrap();
    let read_back = std::fs::read_to_string(&path).unwrap();
    let loaded: AppConfig = serde_json::from_str(&read_back).unwrap();

    assert_eq!(original, loaded);
    let _ = std::fs::remove_dir_all(&dir);
}
