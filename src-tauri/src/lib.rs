//! Tauri 2.x 应用入口（v1.3.0 拆分后）
//!
//! 启动流程：
//! 1. 初始化 core 的日志系统
//! 2. 创建 `IpcState { backend: Arc<Backend> }` 注入 Tauri managed state
//! 3. 启动一个事件转发 task：把 `BackendEvent` 转换成 `app.emit("...", ...)`
//!    （前端的事件监听契约不变：`port-disconnected` / `reconnect-status` / `send-poller-error` / `send-precise-error`）
//! 4. 注册 27 个 Tauri command（同 v1.2.0，前端不用改）

mod ipc;

use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::broadcast;

use ipc::commands::IpcState;
use oh_my_serial_core::{log_init, Backend, BackendEvent, ReconnectPhase};

/// 入口函数（在 main.rs 调用）
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log_init::init();

    let backend = Arc::new(Backend::new());
    let ipc_state = IpcState {
        backend: backend.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ipc_state)
        .invoke_handler(tauri::generate_handler![
            ipc::commands::cmd_list_ports,
            ipc::commands::cmd_open_port,
            ipc::commands::cmd_close_port,
            ipc::commands::cmd_read_buffer,
            ipc::commands::cmd_write_data,
            ipc::commands::cmd_get_buffer_status,
            ipc::commands::cmd_get_connection_status,
            ipc::commands::cmd_queue_add,
            ipc::commands::cmd_queue_remove,
            ipc::commands::cmd_queue_clear,
            ipc::commands::cmd_queue_start_polling,
            ipc::commands::cmd_queue_stop_polling,
            ipc::commands::cmd_queue_status,
            ipc::commands::cmd_start_periodic_send,
            ipc::commands::cmd_stop_periodic_send,
            ipc::commands::cmd_get_log_dir,
            ipc::commands::cmd_read_log_lines,
            ipc::commands::cmd_open_log_dir,
            ipc::commands::cmd_load_config,
            ipc::commands::cmd_save_config,
            ipc::commands::cmd_cancel_reconnect,
            ipc::commands::cmd_list_fonts,
            // v1.2.0 录制
            ipc::commands::cmd_start_recording,
            ipc::commands::cmd_stop_recording,
            ipc::commands::cmd_write_recorder_line,
            ipc::commands::cmd_mark_recorder_event,
            ipc::commands::cmd_is_recording,
        ])
        .setup(move |app| {
            // 事件转发：BackendEvent → Tauri app.emit("...", ...)
            // 前端的事件名（v1.2.0 契约）：port-disconnected / reconnect-status / send-poller-error / send-precise-error
            let mut event_rx = backend.subscribe();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    // 落后（broadcast 缓冲溢出）不等于流结束。定时发送在高频率下
                    // 很容易撑满缓冲，用 `while let Ok(..)` 会让转发永久死掉。
                    let event = match event_rx.recv().await {
                        Ok(event) => event,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            log::warn!("[event-forwarder] 落后 {n} 个事件，跳过");
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    };
                    match event {
                        BackendEvent::PortDisconnected(reason) => {
                            let _ = app_handle.emit("port-disconnected", &reason);
                        }
                        BackendEvent::Reconnect(re) => {
                            let phase_str = match re.phase {
                                ReconnectPhase::Started => "started",
                                ReconnectPhase::Attempt => "attempt",
                                ReconnectPhase::Succeeded => "succeeded",
                                ReconnectPhase::Failed => "failed",
                                ReconnectPhase::Cancelled => "cancelled",
                            };
                            // 沿用 v1.2.0 JSON wire format（前端不变）
                            let payload = serde_json::json!({
                                "state": phase_str,
                                "attempt": re.attempt,
                                "max_attempts": re.max_attempts,
                                "next_delay_ms": re.next_delay_ms,
                                "message": re.message,
                            });
                            let _ = app_handle.emit("reconnect-status", payload);
                        }
                        BackendEvent::SendPollerError(e) => {
                            let _ = app_handle.emit("send-poller-error", &e);
                        }
                        BackendEvent::SendPreciseError(e) => {
                            let _ = app_handle.emit("send-precise-error", &e);
                        }
                        BackendEvent::TxEcho(bytes) => {
                            let _ = app_handle
                                .emit("tx-echo", serde_json::json!({ "bytes": bytes }));
                        }
                        // PortOpened / PortClosed 已被 cmd_open_port / cmd_close_port 的返回值处理，
                        // 不额外 emit（保持 v1.2.0 前端契约）
                        BackendEvent::PortOpened { .. } | BackendEvent::PortClosed => {}
                    }
                }
                log::warn!("[event-forwarder] Backend event stream closed");
            });

            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
