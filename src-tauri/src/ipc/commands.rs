use crate::sender::{SendCommand, SendQueue};
use crate::serial::port::{list_ports, PortInfo};
use crate::serial::ring_buffer::RingBuffer;
use crate::log_init;
use crate::config_impl::{self, AppConfig};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, State};
use std::time::Duration;

/// 全局串口状态
///
/// `ring_buffer` 与 `port_handle` 用 `Arc` 包裹，便于在后台读取线程中持有。
/// `stop_flag` 用于通知后台线程停止读取。
/// `disconnect_flag` 标记设备已拔出，由读取线程设置。
/// `send_queue` 多命令按优先级排序的发送队列。
/// `polling_stop_flag` 用于停止轮询任务。
pub struct SerialState {
    pub ring_buffer: Arc<Mutex<RingBuffer>>,
    pub port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    pub stop_flag: Arc<AtomicBool>,
    pub disconnect_flag: Arc<AtomicBool>,
    pub send_queue: Arc<Mutex<SendQueue>>,
    pub polling_stop_flag: Arc<AtomicBool>,
    pub precise_stop_flag: Arc<AtomicBool>,
    pub reconnect_state: Arc<Mutex<Option<ReconnectHandle>>>,
    /// 录制器：None = 未在录制，Some = 活跃（跨 reader 线程生命周期）
    /// v1.2.0 重连不切文件：断线/重连事件通过 mark_event 写注释行
    pub recorder: Arc<Mutex<Option<crate::recorder::Recorder>>>,
}

/// 自动重连任务句柄（活跃时存进 SerialState）
pub struct ReconnectHandle {
    pub stop_flag: Arc<AtomicBool>,
    pub attempts: Arc<AtomicU32>,
}

/// 自动重连退避序列（秒）：1s → 2s → 4s → 8s → 15s（最多 5 次）
const RECONNECT_BACKOFF_SECS: &[u64] = &[1, 2, 4, 8, 15];

impl Default for SerialState {
    fn default() -> Self {
        Self {
            ring_buffer: Arc::new(Mutex::new(RingBuffer::new(65536))),
            port_handle: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            disconnect_flag: Arc::new(AtomicBool::new(false)),
            send_queue: Arc::new(Mutex::new(SendQueue::new())),
            polling_stop_flag: Arc::new(AtomicBool::new(false)),
            precise_stop_flag: Arc::new(AtomicBool::new(false)),
            reconnect_state: Arc::new(Mutex::new(None)),
            recorder: Arc::new(Mutex::new(None)),
        }
    }
}

/// 连续错误累计阈值：达到后视为断线（防误报）
const DISCONNECT_ERROR_THRESHOLD: u32 = 3;

/// 列出所有可用串口
#[tauri::command]
pub fn cmd_list_ports() -> Result<Vec<PortInfo>, String> {
    Ok(list_ports())
}

/// 打开串口，并启动后台读取线程
///
/// 接收一个 `Channel<Vec<u8>>` 用于零拷贝推送串口数据
/// Channel 通过 clone 移到 reader 线程，reader 用 `channel.send(&payload)` 直推
/// 不再走 `app.emit("serial-data", Vec<u8>)` 的 JSON 序列化路径
#[tauri::command]
pub fn cmd_open_port(
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    state: State<'_, SerialState>,
    app: AppHandle,
    on_data: Channel<Vec<u8>>,
) -> Result<(), String> {
    let mut handle = state.port_handle.lock().map_err(|e| e.to_string())?;

    if handle.is_some() {
        return Err("串口已打开，请先关闭".to_string());
    }

    let port = serialport::new(&port_name, baud_rate)
        .data_bits(match data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            8 => DataBits::Eight,
            _ => return Err("无效的数据位".to_string()),
        })
        .stop_bits(match stop_bits {
            1 => StopBits::One,
            2 => StopBits::Two,
            _ => return Err("无效的停止位".to_string()),
        })
        .parity(match parity.to_uppercase().as_str() {
            "NONE" | "N" => Parity::None,
            "ODD" | "O" => Parity::Odd,
            "EVEN" | "E" => Parity::Even,
            _ => return Err("无效的校验位".to_string()),
        })
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("打开串口失败: {}", e))?;

    *handle = Some(port);

    // 启动后台读取线程
    let ring_buffer = Arc::clone(&state.ring_buffer);
    let port_handle = Arc::clone(&state.port_handle);
    let stop_flag = Arc::clone(&state.stop_flag);
    let disconnect_flag = Arc::clone(&state.disconnect_flag);
    let reconnect_state = Arc::clone(&state.reconnect_state);
    let recorder = Arc::clone(&state.recorder);
    stop_flag.store(false, Ordering::SeqCst);
    disconnect_flag.store(false, Ordering::SeqCst);

    let app2 = app.clone();
    let port_name_for_reader = port_name.clone();
    // Channel 移到 reader 线程（Channel: Clone + Send + Sync）
    let on_data = on_data.clone();
    thread::Builder::new()
        .name("serial-reader".to_string())
        .spawn(move || {
            run_reader_loop(
                ring_buffer,
                port_handle,
                stop_flag,
                disconnect_flag,
                reconnect_state,
                recorder,
                app2,
                port_name_for_reader,
                baud_rate,
                data_bits,
                stop_bits,
                parity,
                on_data,
            );
        })
        .map_err(|e| format!("启动读取线程失败: {}", e))?;

    Ok(())
}

/// 读取线程主体（cmd_open_port 和 schedule_reconnect 成功后都调用）
///
/// 退出条件：stop_flag 置位 / 串口句柄被置 None / 读取到断线信号
/// 退出时：若 auto_reconnect 启用，调度 schedule_reconnect
///
/// 零拷贝：通过 `on_data` Channel 直接发送 `Vec<u8>`，不经 JSON 序列化
/// v1.2.0：断线/重连时通过 recorder 写注释行（如果正在录制）
fn run_reader_loop(
    ring_buffer: Arc<Mutex<RingBuffer>>,
    port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    stop_flag: Arc<AtomicBool>,
    disconnect_flag: Arc<AtomicBool>,
    reconnect_state: Arc<Mutex<Option<ReconnectHandle>>>,
    recorder: Arc<Mutex<Option<crate::recorder::Recorder>>>,
    app: AppHandle,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    on_data: Channel<Vec<u8>>,
) {
    let mut scratch = [0u8; 256];
    let mut last_flush = std::time::Instant::now();
    let mut consecutive_errors: u32 = 0;
    let mut disconnected_naturally = false;
    let mut disconnect_reason = String::new();
    // v1.2.0：断线时刻，用于重连后计算 gap
    let mut disconnect_time: Option<std::time::Instant> = None;

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        // 读一帧数据（分级错误处理）
        let read_n: Option<Result<usize, std::io::Error>> = {
            let mut guard = match port_handle.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            match guard.as_mut() {
                Some(p) => match p.read(&mut scratch) {
                    Ok(n) => Some(Ok(n)),
                    Err(e) => Some(Err(e)),
                },
                None => break,
            }
        };

        match read_n {
            Some(Ok(n)) => {
                if n > 0 {
                    consecutive_errors = 0; // 重置错误计数
                    if let Ok(mut buf) = ring_buffer.lock() {
                        buf.write(&scratch[..n]);
                    }
                }
                // n == 0 (timeout) 也正常继续
            }
            Some(Err(e)) => {
                use std::io::ErrorKind;
                match e.kind() {
                    // 明确断线信号：立即触发
                    ErrorKind::NotConnected | ErrorKind::BrokenPipe => {
                        log::error!("[serial-reader] 设备已断开: {:?}", e);
                        disconnect_flag.store(true, Ordering::SeqCst);
                        disconnect_reason = e.to_string();
                        let _ = app.emit("port-disconnected", &e.to_string());
                        disconnected_naturally = true;
                        disconnect_time = Some(std::time::Instant::now());
                        // v1.2.0：录制中 → 写断线注释
                        if let Ok(mut rg) = recorder.lock() {
                            if let Some(rec) = rg.as_mut() {
                                let _ = rec.mark_event(&format!("设备已断开: {e}"));
                            }
                        }
                        break;
                    }
                    // 超时不视为断线，重置计数
                    ErrorKind::TimedOut => {
                        consecutive_errors = 0;
                    }
                    // 其他错误累计
                    _ => {
                        consecutive_errors += 1;
                        log::warn!(
                            "[serial-reader] 读取错误 ({}/{}): {:?}",
                            consecutive_errors,
                            DISCONNECT_ERROR_THRESHOLD,
                            e
                        );
                        if consecutive_errors >= DISCONNECT_ERROR_THRESHOLD {
                            log::error!("[serial-reader] 连续错误过多，判定为断线");
                            disconnect_flag.store(true, Ordering::SeqCst);
                            disconnect_reason = e.to_string();
                            let _ = app.emit("port-disconnected", &e.to_string());
                            disconnected_naturally = true;
                            disconnect_time = Some(std::time::Instant::now());
                            // v1.2.0：录制中 → 写断线注释
                            if let Ok(mut rg) = recorder.lock() {
                                if let Some(rec) = rg.as_mut() {
                                    let _ = rec.mark_event(&format!("设备已断开: {e}"));
                                }
                            }
                            break;
                        }
                    }
                }
            }
            None => break,
        }

        // 触发条件：满 4KB 或 16ms 定时器溢出
        let should_flush = {
            let buf = match ring_buffer.lock() {
                Ok(b) => b,
                Err(_) => break,
            };
            buf.should_flush() || last_flush.elapsed() >= Duration::from_millis(16)
        };

        if should_flush {
            last_flush = std::time::Instant::now();
            let payload = {
                let mut buf = match ring_buffer.lock() {
                    Ok(b) => b,
                    Err(_) => break,
                };
                buf.drain_all()
            };

            if !payload.is_empty() {
                // 零拷贝：Channel 直接发送 Vec<u8>（底层是 raw binary over IPC pipe）
                if let Err(e) = on_data.send(payload) {
                    log::error!("channel send failed: {:?}", e);
                }
            }
        }

        thread::sleep(Duration::from_millis(2));
    }

    // 退出清理：若非用户主动关闭（disconnected_naturally=true），尝试自动重连
    if disconnected_naturally {
        let cfg = crate::config_impl::load();
        if cfg.auto_reconnect && !stop_flag.load(Ordering::SeqCst) {
            log::info!(
                "[serial-reader] 触发自动重连（原因：{disconnect_reason}，端口：{port_name}）"
            );
            // 重新构造一个轻量级 SerialState 用于 schedule_reconnect
            let st = SerialStateLite {
                ring_buffer,
                port_handle,
                stop_flag: stop_flag.clone(),
                disconnect_flag,
                reconnect_state,
                recorder: recorder.clone(),
            };
            schedule_reconnect(
                st,
                port_name,
                baud_rate,
                data_bits,
                stop_bits,
                parity,
                cfg.reconnect_max_attempts,
                app,
                on_data,
                disconnect_time,
            );
        }
    }
}

/// SerialState 的轻量包装（只含 schedule_reconnect 需要的字段）
pub struct SerialStateLite {
    pub ring_buffer: Arc<Mutex<RingBuffer>>,
    pub port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    pub stop_flag: Arc<AtomicBool>,
    pub disconnect_flag: Arc<AtomicBool>,
    pub reconnect_state: Arc<Mutex<Option<ReconnectHandle>>>,
    /// 录制器：透传给新 reader 线程，断线/重连事件用
    pub recorder: Arc<Mutex<Option<crate::recorder::Recorder>>>,
}

/// 关闭串口，停止后台读取线程
#[tauri::command]
pub fn cmd_close_port(state: State<'_, SerialState>) -> Result<(), String> {
    state.stop_flag.store(true, Ordering::SeqCst);
    let mut handle = state.port_handle.lock().map_err(|e| e.to_string())?;
    *handle = None;
    state.disconnect_flag.store(false, Ordering::SeqCst);
    // 同时取消任何进行中的自动重连
    if let Ok(mut rs) = state.reconnect_state.lock() {
        if let Some(h) = rs.take() {
            h.stop_flag.store(true, Ordering::SeqCst);
            log::info!("[reconnect] 用户主动关闭串口，已取消重连");
        }
    }
    // v1.2.0：用户主动关闭串口 → 自动停止录制（Q13A）
    if let Ok(mut rec_guard) = state.recorder.lock() {
        if let Some(rec) = rec_guard.take() {
            match rec.stop() {
                Ok(summary) => log::info!(
                    "[recorder] 串口关闭，自动停止录制: {} ({} bytes, {} ms)",
                    summary.path.display(),
                    summary.bytes_written,
                    summary.duration_ms
                ),
                Err(e) => log::warn!("[recorder] 串口关闭时停止录制失败: {e}"),
            }
        }
    }
    Ok(())
}

// ==================== 录制功能（v1.2.0）====================

/// 开始录制：打开 path 写入文件，自动写文件头元数据
#[tauri::command]
pub fn cmd_start_recording(
    path: String,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    state: State<'_, SerialState>,
) -> Result<(), String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    if rec_guard.is_some() {
        return Err("已在录制中".to_string());
    }
    let mut rec = crate::recorder::start_recording(std::path::PathBuf::from(&path))
        .map_err(|e| format!("创建录制文件失败: {e}"))?;
    rec.write_header(&port_name, baud_rate, data_bits, stop_bits, &parity)
        .map_err(|e| format!("写文件头失败: {e}"))?;
    log::info!("[recorder] 开始录制: {path}");
    *rec_guard = Some(rec);
    Ok(())
}

/// 停止录制：flush + 关闭 + 返回摘要
#[tauri::command]
pub fn cmd_stop_recording(
    state: State<'_, SerialState>,
) -> Result<crate::recorder::RecorderSummary, String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    let rec = rec_guard.take().ok_or("未在录制")?;
    let summary = rec.stop().map_err(|e| format!("停止录制失败: {e}"))?;
    log::info!(
        "[recorder] 停止录制: {} ({} bytes, {} ms)",
        summary.path.display(),
        summary.bytes_written,
        summary.duration_ms
    );
    Ok(summary)
}

/// 写入一行纯文本到录制文件（前端 Terminal writeData 调用）
/// 如果未在录制，静默成功（no-op）
#[tauri::command]
pub fn cmd_write_recorder_line(line: String, state: State<'_, SerialState>) -> Result<(), String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    if let Some(rec) = rec_guard.as_mut() {
        rec.write_line(&line).map_err(|e| format!("写入失败: {e}"))?;
    }
    Ok(())
}

/// 写入一行注释（系统消息 + 断线/重连事件）
/// 如果未在录制，静默成功（no-op）
#[tauri::command]
pub fn cmd_mark_recorder_event(
    text: String,
    state: State<'_, SerialState>,
) -> Result<(), String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    if let Some(rec) = rec_guard.as_mut() {
        rec.mark_event(&text).map_err(|e| format!("写入失败: {e}"))?;
    }
    Ok(())
}

/// 查询当前是否在录制（用于启动时恢复状态）
#[tauri::command]
pub fn cmd_is_recording(state: State<'_, SerialState>) -> Result<bool, String> {
    let rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    Ok(rec_guard.is_some())
}

// ==================== 自动重连 ====================

/// 自动重连事件（推送给前端的进度）
#[derive(serde::Serialize, Clone)]
pub struct ReconnectStatus {
    pub state: String, // "started" | "attempt" | "succeeded" | "failed" | "cancelled"
    pub attempt: u32,
    pub max_attempts: u32,
    pub next_delay_ms: u64, // 仅 attempt 时有效
    pub message: String,
}

/// 尝试重连给定端口（用 last_port + 上次的波特率）
fn try_reconnect_open(
    port_name: &str,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: &str,
) -> Result<Box<dyn SerialPort>, String> {
    let db = match data_bits {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        8 => DataBits::Eight,
        _ => return Err(format!("无效的数据位: {data_bits}")),
    };
    let sb = match stop_bits {
        1 => StopBits::One,
        2 => StopBits::Two,
        _ => return Err(format!("无效的停止位: {stop_bits}")),
    };
    let pa = match parity.to_uppercase().as_str() {
        "NONE" | "N" => Parity::None,
        "ODD" | "O" => Parity::Odd,
        "EVEN" | "E" => Parity::Even,
        _ => return Err(format!("无效的校验位: {parity}")),
    };
    serialport::new(port_name, baud_rate)
        .data_bits(db)
        .stop_bits(sb)
        .parity(pa)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("打开失败: {e}"))
}

/// 启动自动重连线程（被 reader 线程在检测到断线时调用）
///
/// 设计：开新线程跑退避循环，每次成功就 spawn 新 reader；失败就递增 attempts；
/// 达到 max_attempts 或 stop_flag 置位就退出。
///
/// `on_data` 沿用调用者（reader 线程）持有的 channel，确保重连后的 reader 仍能推送数据
/// v1.2.0：重连成功时通过 recorder 写 "重连成功 (gap Xs)" 注释
pub fn schedule_reconnect(
    state: SerialStateLite,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    max_attempts: u32,
    app: AppHandle,
    on_data: Channel<Vec<u8>>,
    disconnect_time: Option<std::time::Instant>,
) {
    // 若已有重连任务在跑，先取消
    if let Ok(mut rs) = state.reconnect_state.lock() {
        if let Some(prev) = rs.take() {
            prev.stop_flag.store(true, Ordering::SeqCst);
        }
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let attempts = Arc::new(AtomicU32::new(0));

    // 注册句柄（让用户能取消）
    if let Ok(mut rs) = state.reconnect_state.lock() {
        *rs = Some(ReconnectHandle {
            stop_flag: Arc::clone(&stop_flag),
            attempts: Arc::clone(&attempts),
        });
    }

    let _ = app.emit(
        "reconnect-status",
        ReconnectStatus {
            state: "started".into(),
            attempt: 0,
            max_attempts,
            next_delay_ms: 0,
            message: format!("已断开，准备重连 {port_name}"),
        },
    );
    log::info!("[reconnect] 启动：{} 最多 {} 次", port_name, max_attempts);

    let ring_buffer = Arc::clone(&state.ring_buffer);
    let port_handle = Arc::clone(&state.port_handle);
    let serial_stop = Arc::clone(&state.stop_flag);
    let disconnect_flag = Arc::clone(&state.disconnect_flag);
    let reconnect_state = Arc::clone(&state.reconnect_state);
    let recorder = Arc::clone(&state.recorder);

    thread::Builder::new()
        .name("reconnect-loop".to_string())
        .spawn(move || {
            // 退避序列循环
            for (idx, &backoff_sec) in RECONNECT_BACKOFF_SECS.iter().enumerate() {
                if stop_flag.load(Ordering::SeqCst) {
                    log::info!("[reconnect] 已取消");
                    return;
                }

                // 退避
                let _ = app.emit(
                    "reconnect-status",
                    ReconnectStatus {
                        state: "attempt".into(),
                        attempt: (idx + 1) as u32,
                        max_attempts,
                        next_delay_ms: backoff_sec * 1000,
                        message: format!(
                            "{} 秒后第 {} 次重试 {}",
                            backoff_sec,
                            idx + 1,
                            port_name
                        ),
                    },
                );
                attempts.store((idx + 1) as u32, Ordering::SeqCst);

                for _ in 0..(backoff_sec * 10) {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                // 尝试打开
                match try_reconnect_open(&port_name, baud_rate, data_bits, stop_bits, &parity) {
                    Ok(port) => {
                        log::info!(
                            "[reconnect] 第 {} 次重连成功（{}）",
                            idx + 1,
                            port_name
                        );

                        // 写入 port_handle
                        {
                            let mut h = match port_handle.lock() {
                                Ok(h) => h,
                                Err(_) => return,
                            };
                            *h = Some(port);
                        }

                        // 重置标志
                        serial_stop.store(false, Ordering::SeqCst);
                        disconnect_flag.store(false, Ordering::SeqCst);

                        // 通知前端
                        let _ = app.emit(
                            "reconnect-status",
                            ReconnectStatus {
                                state: "succeeded".into(),
                                attempt: (idx + 1) as u32,
                                max_attempts,
                                next_delay_ms: 0,
                                message: format!("重连成功：{port_name}"),
                            },
                        );
                        // 兼容旧的 port-opened 事件
                        let _ = app.emit("port-opened", &port_name);

                        // v1.2.0：录制中 → 写重连成功注释（含 gap 时长）
                        if let Ok(mut rg) = recorder.lock() {
                            if let Some(rec) = rg.as_mut() {
                                let gap_text = match disconnect_time {
                                    Some(t) => {
                                        let secs = t.elapsed().as_secs_f64();
                                        format!("重连成功 (gap {:.3}s)", secs)
                                    }
                                    None => "重连成功".to_string(),
                                };
                                let _ = rec.mark_event(&gap_text);
                            }
                        }

                        // 清空句柄（让用户能开新一轮重连）
                        if let Ok(mut rs) = reconnect_state.lock() {
                            *rs = None;
                        }

                        // 启动新 reader 线程（沿用本次重连持有的 channel）
                        let rb = Arc::clone(&ring_buffer);
                        let ph = Arc::clone(&port_handle);
                        let sf = Arc::clone(&serial_stop);
                        let df = Arc::clone(&disconnect_flag);
                        let rs2 = Arc::clone(&reconnect_state);
                        let rec2 = Arc::clone(&recorder);
                        let app2 = app.clone();
                        let pn = port_name.clone();
                        let br = baud_rate;
                        let db2 = data_bits;
                        let sb2 = stop_bits;
                        let pa2 = parity.clone();
                        let on_data2 = on_data.clone();
                        let _ = thread::Builder::new()
                            .name("serial-reader".to_string())
                            .spawn(move || {
                                run_reader_loop(rb, ph, sf, df, rs2, rec2, app2, pn, br, db2, sb2, pa2, on_data2);
                            });
                        return;
                    }
                    Err(e) => {
                        log::warn!("[reconnect] 第 {} 次失败：{}", idx + 1, e);
                    }
                }
            }

            // 全部失败
            log::error!("[reconnect] {} 次重连全部失败", max_attempts);
            let _ = app.emit(
                "reconnect-status",
                ReconnectStatus {
                    state: "failed".into(),
                    attempt: max_attempts,
                    max_attempts,
                    next_delay_ms: 0,
                    message: format!("{port_name} 重连失败，已放弃"),
                },
            );
            if let Ok(mut rs) = reconnect_state.lock() {
                *rs = None;
            }
        })
        .expect("启动重连线程失败");
}

/// 取消进行中的自动重连
#[tauri::command]
pub fn cmd_cancel_reconnect(state: State<'_, SerialState>) -> Result<(), String> {
    if let Ok(mut rs) = state.reconnect_state.lock() {
        if let Some(h) = rs.take() {
            h.stop_flag.store(true, Ordering::SeqCst);
            log::info!("[reconnect] 用户主动取消");
        }
    }
    Ok(())
}

/// 获取连接状态
#[tauri::command]
pub fn cmd_get_connection_status(state: State<'_, SerialState>) -> Result<ConnectionStatus, String> {
    let is_open = state.port_handle.lock().map_err(|e| e.to_string())?.is_some();
    let disconnected = state.disconnect_flag.load(Ordering::SeqCst);
    Ok(ConnectionStatus {
        is_open,
        disconnected,
    })
}

/// 连接状态
#[derive(serde::Serialize)]
pub struct ConnectionStatus {
    pub is_open: bool,
    pub disconnected: bool,
}

/// 读取缓冲区数据
#[tauri::command]
pub fn cmd_read_buffer(state: State<'_, SerialState>, len: usize) -> Result<Vec<u8>, String> {
    let mut buf = state.ring_buffer.lock().map_err(|e| e.to_string())?;
    Ok(buf.read(len))
}

/// 写入数据到串口
#[tauri::command]
pub fn cmd_write_data(data: Vec<u8>, state: State<'_, SerialState>) -> Result<(), String> {
    let mut handle = state.port_handle.lock().map_err(|e| e.to_string())?;

    let port = handle.as_mut().ok_or("串口未打开")?;
    port.write(&data).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

/// 获取缓冲区状态
#[tauri::command]
pub fn cmd_get_buffer_status(state: State<'_, SerialState>) -> Result<BufferStatus, String> {
    let buf = state.ring_buffer.lock().map_err(|e| e.to_string())?;
    Ok(BufferStatus {
        data_len: buf.data_len(),
        water_level: buf.water_level(),
        backpressure: format!("{:?}", buf.backpressure_state()),
        overflow_count: buf.overflow_count(),
    })
}

/// 缓冲区状态
#[derive(serde::Serialize)]
pub struct BufferStatus {
    pub data_len: usize,
    pub water_level: f32,
    pub backpressure: String,
    pub overflow_count: usize,
}

// ==================== SendQueue IPC ====================

/// 向发送队列添加一条命令
#[tauri::command]
pub fn cmd_queue_add(
    state: State<'_, SerialState>,
    id: String,
    content: Vec<u8>,
    priority: u8,
    interval_ms: u64,
) -> Result<(), String> {
    let mut q = state.send_queue.lock().map_err(|e| e.to_string())?;
    q.add(SendCommand {
        id,
        content,
        priority,
        interval_ms,
    });
    Ok(())
}

/// 从发送队列移除一条命令
#[tauri::command]
pub fn cmd_queue_remove(state: State<'_, SerialState>, id: String) -> Result<(), String> {
    let mut q = state.send_queue.lock().map_err(|e| e.to_string())?;
    q.remove(&id);
    Ok(())
}

/// 清空发送队列
#[tauri::command]
pub fn cmd_queue_clear(state: State<'_, SerialState>) -> Result<(), String> {
    let mut q = state.send_queue.lock().map_err(|e| e.to_string())?;
    q.clear();
    Ok(())
}

/// 启动后台轮询任务
#[tauri::command]
pub fn cmd_queue_start_polling(
    state: State<'_, SerialState>,
    app: AppHandle,
) -> Result<(), String> {
    let send_queue = Arc::clone(&state.send_queue);
    let port_handle = Arc::clone(&state.port_handle);
    let polling_stop_flag = Arc::clone(&state.polling_stop_flag);

    // 若已在跑，幂等返回
    if !polling_stop_flag.load(Ordering::SeqCst) {
        return Ok(());
    }
    polling_stop_flag.store(false, Ordering::SeqCst);

    let mut queue = send_queue.lock().map_err(|e| e.to_string())?;
    queue.start_polling();
    drop(queue);

    thread::Builder::new()
        .name("send-poller".to_string())
        .spawn(move || {
            loop {
                if polling_stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                // 取下一条命令
                let next = {
                    let q = match send_queue.lock() {
                        Ok(q) => q,
                        Err(_) => break,
                    };
                    if !q.is_polling() {
                        break;
                    }
                    q.next_command().cloned()
                };

                if let Some(cmd) = next {
                    // 写入串口（try_lock + 短暂重试，避免与 reader 线程长持锁）
                    let mut attempts = 0;
                    let write_result = loop {
                        match port_handle.try_lock() {
                            Ok(mut guard) => {
                                if let Some(port) = guard.as_mut() {
                                    break port.write_all(&cmd.content);
                                } else {
                                    break Err(std::io::Error::new(
                                        std::io::ErrorKind::NotConnected,
                                        "串口未打开",
                                    ));
                                }
                            }
                            Err(_) => {
                                attempts += 1;
                                if attempts > 50 {
                                    break Err(std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        "无法获取串口锁",
                                    ));
                                }
                                thread::sleep(Duration::from_millis(2));
                            }
                        }
                    };

                    if let Err(e) = write_result {
                        log::error!("[send-poller] 写入失败: {:?}", e);
                        let _ = app.emit("send-poller-error", &e.to_string());
                        break;
                    }

                    // 等待 interval
                    thread::sleep(Duration::from_millis(cmd.interval_ms));
                } else {
                    // 队列为空，短暂等待后重试
                    thread::sleep(Duration::from_millis(50));
                }
            }

            // 退出时标记停止
            if let Ok(mut q) = send_queue.lock() {
                q.stop_polling();
            }
            polling_stop_flag.store(true, Ordering::SeqCst);
        })
        .map_err(|e| format!("启动轮询线程失败: {}", e))?;

    Ok(())
}

/// 停止后台轮询任务
#[tauri::command]
pub fn cmd_queue_stop_polling(state: State<'_, SerialState>) -> Result<(), String> {
    state.polling_stop_flag.store(true, Ordering::SeqCst);
    if let Ok(mut q) = state.send_queue.lock() {
        q.stop_polling();
    }
    Ok(())
}

/// 获取队列状态
#[tauri::command]
pub fn cmd_queue_status(
    state: State<'_, SerialState>,
) -> Result<QueueStatus, String> {
    let q = state.send_queue.lock().map_err(|e| e.to_string())?;
    Ok(QueueStatus {
        count: q.get_commands().len(),
        is_polling: q.is_polling(),
    })
}

// ==================== Periodic Send (PreciseSender 集成) ====================

/// 启动单 payload 周期发送
///
/// 与 SendQueue 多命令不同：单 payload 循环，周期固定
/// 内部使用 `state.precise_stop_flag` 控制停止
#[tauri::command]
pub fn cmd_start_periodic_send(
    state: State<'_, SerialState>,
    payload: Vec<u8>,
    interval_ms: u64,
    app: AppHandle,
) -> Result<(), String> {
    use std::time::Instant;

    let port_handle = Arc::clone(&state.port_handle);
    let precise_stop_flag = Arc::clone(&state.precise_stop_flag);
    precise_stop_flag.store(false, Ordering::SeqCst);

    // 清空 SendQueue，避免与周期性发送冲突
    {
        let mut q = state.send_queue.lock().map_err(|e| e.to_string())?;
        q.clear();
    }

    thread::Builder::new()
        .name("send-precise".to_string())
        .spawn(move || {
            let interval_dur = Duration::from_millis(interval_ms);
            let mut next_tick = Instant::now() + interval_dur;

            loop {
                if precise_stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                let now = Instant::now();
                if now < next_tick {
                    thread::sleep(next_tick - now);
                }
                next_tick += interval_dur;

                // 写入（try_lock + 短暂重试）
                let mut attempts = 0u32;
                let write_result = loop {
                    match port_handle.try_lock() {
                        Ok(mut guard) => {
                            if let Some(port) = guard.as_mut() {
                                break port.write_all(&payload);
                            } else {
                                break Err(std::io::Error::new(
                                    std::io::ErrorKind::NotConnected,
                                    "串口未打开",
                                ));
                            }
                        }
                        Err(_) => {
                            attempts += 1;
                            if attempts > 50 {
                                break Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    "无法获取串口锁",
                                ));
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                    }
                };

                if let Err(e) = write_result {
                    log::error!("[send-precise] 写入失败: {:?}", e);
                    let _ = app.emit("send-precise-error", &e.to_string());
                    break;
                }
            }
        })
        .map_err(|e| format!("启动精确发送失败: {}", e))?;

    Ok(())
}

/// 停止单 payload 周期发送
#[tauri::command]
pub fn cmd_stop_periodic_send(state: State<'_, SerialState>) -> Result<(), String> {
    state.precise_stop_flag.store(true, Ordering::SeqCst);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct QueueStatus {
    pub count: usize,
    pub is_polling: bool,
}

// ==================== 日志 ====================

/// 返回当前日志目录路径（前端用：状态栏展示、打开目录按钮）
#[tauri::command]
pub fn cmd_get_log_dir() -> Result<String, String> {
    Ok(log_init::log_dir_str())
}

/// 读最近 N 行日志（前端 LogPanel 用）
///
/// offset=0 → 最新 limit 行；offset=1 → 倒数第 2 个 limit 行
/// level_filter=Some("WARN") → 只返 WARN 及以上
#[tauri::command]
pub fn cmd_read_log_lines(
    offset: u32,
    limit: u32,
    level_filter: Option<String>,
) -> Result<Vec<log_init::LogLine>, String> {
    log_init::read_recent_lines(offset, limit, level_filter.as_deref())
        .map_err(|e| format!("读日志失败: {e}"))
}

/// 打开日志目录（系统资源管理器）
#[tauri::command]
pub fn cmd_open_log_dir() -> Result<(), String> {
    let dir = log_init::log_dir_path();
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

/// 加载应用配置（启动时调用一次）
#[tauri::command]
pub fn cmd_load_config() -> AppConfig {
    let cfg = config_impl::load();
    log::info!(
        "📋 配置已加载：last_port={:?}, baud_rate={}, theme={}",
        cfg.last_port, cfg.baud_rate, cfg.theme
    );
    cfg
}

/// 保存应用配置（任何设置变更后调用）
#[tauri::command]
pub fn cmd_save_config(config: AppConfig) -> Result<(), String> {
    config_impl::save(&config)
}

/// 列出系统已安装的字体 family（前端字体选择器用）
#[tauri::command]
pub fn cmd_list_fonts() -> Vec<crate::fonts::FontInfo> {
    log::info!("📋 列出系统字体");
    crate::fonts::list_mono_fonts()
}
