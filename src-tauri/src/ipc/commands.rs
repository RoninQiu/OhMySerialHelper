use crate::serial::port::{list_ports, PortInfo};
use crate::serial::ring_buffer::RingBuffer;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::sync::Mutex;
use tauri::State;

/// 串口状态
pub struct SerialState {
    pub ring_buffer: Mutex<RingBuffer>,
    pub port_handle: Mutex<Option<Box<dyn SerialPort>>>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            ring_buffer: Mutex::new(RingBuffer::new(65536)),
            port_handle: Mutex::new(None),
        }
    }
}

/// 列出所有可用串口
#[tauri::command]
pub fn cmd_list_ports() -> Result<Vec<PortInfo>, String> {
    Ok(list_ports())
}

/// 打开串口
#[tauri::command]
pub fn cmd_open_port(
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    state: State<'_, SerialState>,
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
        .open()
        .map_err(|e| format!("打开串口失败: {}", e))?;

    *handle = Some(port);
    Ok(())
}

/// 关闭串口
#[tauri::command]
pub fn cmd_close_port(state: State<'_, SerialState>) -> Result<(), String> {
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
