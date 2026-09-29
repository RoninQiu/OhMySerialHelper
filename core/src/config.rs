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
    load_from(&config_path()).unwrap_or_default()
}

/// 与 `load()` 读同一个文件，但**读不到或解析不了就返回 `None`**。
///
/// `load()` 分不清「用户没有配置」和「配置坏了」，两种情况都悄悄回落到
/// 默认值。启动时这样没问题（用户第一次用就是默认值），但**写盘前的
/// 重新装载**不行：拿一份坏文件的默认值当 base 再存回去，会把用户整套
/// 设置重置掉。调用方拿到 `None` 就该跳过这次写盘。
pub fn load_checked() -> Option<AppConfig> {
    load_from(&config_path())
}

fn load_from(path: &Path) -> Option<AppConfig> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                log::warn!("配置文件解析失败（{}）: {}", path.display(), e);
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            log::warn!("读取配置失败（{}）: {}", path.display(), e);
            None
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("oms-config-test");
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn load_from_missing_file_is_none() {
        let p = tmp_path("does-not-exist-xyz.json");
        let _ = fs::remove_file(&p);
        assert!(
            load_from(&p).is_none(),
            "文件不存在必须返回 None 而不是默认值"
        );
    }

    #[test]
    fn load_from_malformed_json_is_none() {
        // 这是 load_checked 存在的全部理由：坏文件绝不能被默认值顶替，
        // 否则写盘时会把用户整套设置重置掉。
        let p = tmp_path("malformed.json");
        fs::write(&p, "{ this is not json ").unwrap();
        assert!(load_from(&p).is_none());
    }

    #[test]
    fn load_from_valid_file_returns_config() {
        let p = tmp_path("valid.json");
        let cfg = AppConfig {
            baud_rate: 9600,
            ..Default::default()
        };
        fs::write(&p, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let got = load_from(&p).expect("合法配置应能读出");
        assert_eq!(got.baud_rate, 9600);
    }

    #[test]
    fn load_falls_back_to_default_where_load_checked_gives_up() {
        // 同一个坏文件：load() 静默给默认值，load_checked() 明确说「不知道」。
        // 两种行为都得在，前端启动路径用前者、写盘路径必须用后者。
        let p = tmp_path("fallback.json");
        fs::write(&p, "not json at all").unwrap();
        assert_eq!(load_from(&p).unwrap_or_default().baud_rate, 115200);
        assert!(load_from(&p).is_none());
    }
}
