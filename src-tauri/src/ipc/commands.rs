use crate::sender::{SendCommand, SendQueue};
use crate::serial::port::{list_ports, PortInfo};
use crate::serial::ring_buffer::RingBuffer;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
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
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            ring_buffer: Arc::new(Mutex::new(RingBuffer::new(65536))),
            port_handle: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            disconnect_flag: Arc::new(AtomicBool::new(false)),
            send_queue: Arc::new(Mutex::new(SendQueue::new())),
            polling_stop_flag: Arc::new(AtomicBool::new(false)),
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
#[tauri::command]
pub fn cmd_open_port(
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    state: State<'_, SerialState>,
    app: AppHandle,
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
    stop_flag.store(false, Ordering::SeqCst);
    disconnect_flag.store(false, Ordering::SeqCst);

    thread::Builder::new()
        .name("serial-reader".to_string())
        .spawn(move || {
            let mut scratch = [0u8; 256];
            let mut last_flush = std::time::Instant::now();
            let mut consecutive_errors: u32 = 0;

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
                                eprintln!("[serial-reader] 设备已断开: {:?}", e);
                                disconnect_flag.store(true, Ordering::SeqCst);
                                if let Err(emit_err) = app.emit("port-disconnected", &e.to_string()) {
                                    eprintln!("emit port-disconnected failed: {:?}", emit_err);
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
                                eprintln!(
                                    "[serial-reader] 读取错误 ({}/{}): {:?}",
                                    consecutive_errors, DISCONNECT_ERROR_THRESHOLD, e
                                );
                                if consecutive_errors >= DISCONNECT_ERROR_THRESHOLD {
                                    eprintln!("[serial-reader] 连续错误过多，判定为断线");
                                    disconnect_flag.store(true, Ordering::SeqCst);
                                    if let Err(emit_err) = app.emit("port-disconnected", &e.to_string()) {
                                        eprintln!("emit port-disconnected failed: {:?}", emit_err);
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
                        if let Err(e) = app.emit("serial-data", &payload) {
                            eprintln!("emit serial-data failed: {:?}", e);
                        }
                    }
                }

                thread::sleep(Duration::from_millis(2));
            }
        })
        .map_err(|e| format!("启动读取线程失败: {}", e))?;

    Ok(())
}

/// 关闭串口，停止后台读取线程
#[tauri::command]
pub fn cmd_close_port(state: State<'_, SerialState>) -> Result<(), String> {
    state.stop_flag.store(true, Ordering::SeqCst);
    let mut handle = state.port_handle.lock().map_err(|e| e.to_string())?;
    *handle = None;
    state.disconnect_flag.store(false, Ordering::SeqCst);
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
                        eprintln!("[send-poller] 写入失败: {:?}", e);
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

#[derive(serde::Serialize)]
pub struct QueueStatus {
    pub count: usize,
    pub is_polling: bool,
}
