//! 配置持久化的实际实现（与 `config` 公开 API 一致）
//!
//! 拆出来单独一个文件，方便通过 `pub mod config` 重新暴露给 integration test
//! （lib test 在 Windows 上有 STATUS_ENTRYPOINT_NOT_FOUND 问题）。
//!
//! 真实 API（`load` / `save` / `AppConfig`）从这个模块导出。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
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
    /// v1.2.0 录制：默认保存路径（空 = 每次弹对话框）
    #[serde(default)]
    pub default_capture_path: String,
    /// v1.2.0 录制：每次录制时弹文件对话框（默认 true）
    #[serde(default = "default_prompt_save_dialog")]
    pub prompt_save_dialog: bool,
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
fn default_font_size() -> u32 { 14 }
fn default_font_family() -> String { "system-default".to_string() }
fn default_prompt_save_dialog() -> bool { true }

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
            font_size: default_font_size(),
            font_family: default_font_family(),
            default_capture_path: String::new(),
            prompt_save_dialog: default_prompt_save_dialog(),
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata)
            .join("com.ohmyserial.app")
            .join("config.json");
    }
    PathBuf::from("config.json")
}

fn ensure_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn load() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!("配置文件解析失败（{}），使用默认值: {}", path.display(), e);
                AppConfig::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
        Err(e) => {
            log::warn!("读取配置失败（{}），使用默认值: {}", path.display(), e);
            AppConfig::default()
        }
    }
}

pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let path = config_path();
    ensure_dir(&path).map_err(|e| format!("创建配置目录失败: {e}"))?;
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("序列化配置失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("写临时配置失败: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("重命名配置失败: {e}"))?;
    log::info!("💾 配置已保存：{}", path.display());
    Ok(())
}
