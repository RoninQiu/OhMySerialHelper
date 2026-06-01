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
pub struct SerialState {
    pub ring_buffer: Arc<Mutex<RingBuffer>>,
    pub port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    pub stop_flag: Arc<AtomicBool>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            ring_buffer: Arc::new(Mutex::new(RingBuffer::new(65536))),
            port_handle: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

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
    stop_flag.store(false, Ordering::SeqCst);

    thread::Builder::new()
        .name("serial-reader".to_string())
        .spawn(move || {
            let mut scratch = [0u8; 256];
            let mut last_flush = std::time::Instant::now();

            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                // 读一帧数据
                let read_result = {
                    let mut guard = match port_handle.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    match guard.as_mut() {
                        Some(p) => p.read(&mut scratch).map(|n| n).ok(),
                        None => break,
                    }
                };

                if let Some(n) = read_result {
                    if n > 0 {
                        if let Ok(mut buf) = ring_buffer.lock() {
                            buf.write(&scratch[..n]);
                        }
                    }
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
    Ok(())
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
