//! Tauri 命令薄壳（27 个），全部 1-3 行 delegate 给 `oh_my_serial_core::Backend`
//!
//! 关联 Tauri 原语的处理都收敛在 `cmd_open_port` 一处：
//! - 把 `Channel<Vec<u8>>` 用 mpsc 通道桥接到 Backend（forwarder task 自动管理生命周期）

use oh_my_serial_core::{
    list_mono_fonts, AppConfig, Backend, FontInfo, OpenPortOptions, SendCommand,
};
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

/// Tauri-managed 应用状态（一个 Backend 实例）
#[derive(Default)]
pub struct IpcState {
    pub backend: Arc<Backend>,
}

// ==================== Serial 基础 ====================

/// 列出所有可用串口
#[tauri::command]
pub fn cmd_list_ports(state: State<'_, IpcState>) -> Vec<oh_my_serial_core::PortInfo> {
    state.backend.list_ports()
}

/// 打开串口，并启动后台读取线程
///
/// 这里把 Tauri 的 `Channel<Vec<u8>>` 桥接到 Backend 的 `mpsc::Sender<Vec<u8>>`：
/// 1. 创建一个内部 mpsc 通道
/// 2. 启动一个 forwarder task：mpsc::Receiver → Tauri Channel（保持 v0.6.0 的零拷贝契约）
/// 3. 把 mpsc::Sender 给 Backend，reader 线程持有
///
/// close_port 时 stop_flag → reader 退出 → data_tx drop → mpsc 关 → forwarder 退出
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cmd_open_port(
    app: AppHandle,
    state: State<'_, IpcState>,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    on_data: Channel<Vec<u8>>,
) -> Result<(), String> {
    let backend = state.backend.clone();

    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    let on_data_clone = on_data.clone();

    // forwarder: mpsc Receiver → Tauri Channel (零拷贝 Vec<u8>)
    tokio::spawn(async move {
        while let Some(bytes) = data_rx.recv().await {
            if let Err(e) = on_data_clone.send(bytes) {
                log::warn!("[forwarder] tauri channel send failed: {:?}", e);
                break;
            }
        }
        log::info!("[forwarder] exited（端口已关闭或 channel 断开）");
    });

    backend
        .open_port(
            OpenPortOptions {
                port_name,
                baud_rate,
                data_bits,
                stop_bits,
                parity,
            },
            data_tx,
        )
        .map_err(|e| e.to_string())?;

    let _ = app; // 抑制未用警告（预留：未来可能要发 open 事件给前端）
    Ok(())
}

/// 关闭串口
#[tauri::command]
pub fn cmd_close_port(state: State<'_, IpcState>) -> Result<(), String> {
    state.backend.close_port().map_err(|e| e.to_string())
}

/// 读缓冲区数据
#[tauri::command]
pub fn cmd_read_buffer(state: State<'_, IpcState>, len: usize) -> Result<Vec<u8>, String> {
    state.backend.read_buffer(len).map_err(|e| e.to_string())
}

/// 写数据
#[tauri::command]
pub fn cmd_write_data(state: State<'_, IpcState>, data: Vec<u8>) -> Result<(), String> {
    state.backend.write_data(&data).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
pub struct BufferStatus {
    pub data_len: usize,
    pub water_level: f32,
    pub backpressure: String,
    pub overflow_count: usize,
}

/// 缓冲区状态
#[tauri::command]
pub fn cmd_get_buffer_status(state: State<'_, IpcState>) -> Result<BufferStatus, String> {
    let s = state.backend.buffer_status().map_err(|e| e.to_string())?;
    Ok(BufferStatus {
        data_len: s.data_len,
        water_level: s.water_level,
        backpressure: s.backpressure,
        overflow_count: s.overflow_count,
    })
}

#[derive(serde::Serialize, Clone)]
pub struct ConnectionStatus {
    pub is_open: bool,
    pub disconnected: bool,
}

/// 连接状态
#[tauri::command]
pub fn cmd_get_connection_status(state: State<'_, IpcState>) -> Result<ConnectionStatus, String> {
    let s = state
        .backend
        .connection_status()
        .map_err(|e| e.to_string())?;
    Ok(ConnectionStatus {
        is_open: s.is_open,
        disconnected: s.disconnected,
    })
}

// ==================== 重连 ====================

/// 取消进行中的自动重连
#[tauri::command]
pub fn cmd_cancel_reconnect(state: State<'_, IpcState>) -> Result<(), String> {
    state.backend.cancel_reconnect().map_err(|e| e.to_string())
}

// ==================== SendQueue ====================

#[tauri::command]
pub fn cmd_queue_add(
    state: State<'_, IpcState>,
    id: String,
    content: Vec<u8>,
    priority: u8,
    interval_ms: u64,
) -> Result<(), String> {
    state
        .backend
        .queue_add(SendCommand {
            id,
            content,
            priority,
            interval_ms,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_queue_remove(state: State<'_, IpcState>, id: String) -> Result<(), String> {
    state.backend.queue_remove(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_queue_clear(state: State<'_, IpcState>) -> Result<(), String> {
    state.backend.queue_clear().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_queue_start_polling(state: State<'_, IpcState>) -> Result<(), String> {
    state
        .backend
        .queue_start_polling()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_queue_stop_polling(state: State<'_, IpcState>) -> Result<(), String> {
    state
        .backend
        .queue_stop_polling()
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
pub struct QueueStatus {
    pub count: usize,
    pub is_polling: bool,
}

#[tauri::command]
pub fn cmd_queue_status(state: State<'_, IpcState>) -> Result<QueueStatus, String> {
    let q = state.backend.queue_status().map_err(|e| e.to_string())?;
    Ok(QueueStatus {
        count: q.count,
        is_polling: q.is_polling,
    })
}

// ==================== Periodic Send ====================

#[tauri::command]
pub fn cmd_start_periodic_send(
    state: State<'_, IpcState>,
    payload: Vec<u8>,
    interval_ms: u64,
) -> Result<(), String> {
    state
        .backend
        .start_periodic_send(payload, interval_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_stop_periodic_send(state: State<'_, IpcState>) -> Result<(), String> {
    state
        .backend
        .stop_periodic_send()
        .map_err(|e| e.to_string())
}

// ==================== 录制（v1.2.0）====================

#[tauri::command]
pub fn cmd_start_recording(
    state: State<'_, IpcState>,
    path: String,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
) -> Result<(), String> {
    state
        .backend
        .start_recording(
            std::path::PathBuf::from(path),
            port_name,
            baud_rate,
            data_bits,
            stop_bits,
            parity,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_stop_recording(
    state: State<'_, IpcState>,
) -> Result<oh_my_serial_core::RecorderSummary, String> {
    state.backend.stop_recording().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_write_recorder_line(state: State<'_, IpcState>, line: String) -> Result<(), String> {
    state
        .backend
        .write_recorder_line(&line)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_mark_recorder_event(state: State<'_, IpcState>, text: String) -> Result<(), String> {
    state
        .backend
        .mark_recorder_event(&text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_is_recording(state: State<'_, IpcState>) -> Result<bool, String> {
    state.backend.is_recording().map_err(|e| e.to_string())
}

// ==================== 日志 ====================

#[tauri::command]
pub fn cmd_get_log_dir() -> String {
    oh_my_serial_core::log_init::log_dir_str()
}

#[tauri::command]
pub fn cmd_read_log_lines(
    offset: u32,
    limit: u32,
    level_filter: Option<String>,
) -> Result<Vec<oh_my_serial_core::log_init::LogLine>, String> {
    oh_my_serial_core::log_init::read_recent_lines(offset, limit, level_filter.as_deref())
        .map_err(|e| format!("读日志失败: {e}"))
}

/// 打开日志目录（系统资源管理器）
#[tauri::command]
pub fn cmd_open_log_dir() -> Result<(), String> {
    let dir = oh_my_serial_core::log_init::log_dir_path();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("打开目录失败: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("打开目录失败: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("打开目录失败: {e}"))?;
    }
    Ok(())
}

// ==================== 配置 ====================

#[tauri::command]
pub fn cmd_load_config() -> AppConfig {
    oh_my_serial_core::config::load()
}

#[tauri::command]
pub fn cmd_save_config(config: AppConfig) -> Result<(), String> {
    oh_my_serial_core::config::save(&config)
}

// ==================== 字体 ====================

#[tauri::command]
pub fn cmd_list_fonts() -> Vec<FontInfo> {
    list_mono_fonts()
}
