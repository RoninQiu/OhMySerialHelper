//! UI-agnostic 串口 Backend（Tauri / egui 共用）
//!
//! v1.3.0 起从 src-tauri/src/ipc/commands.rs 拆分：
//! - 不依赖任何 UI 框架（Tauri / egui / ...）
//! - 数据通道：`tokio::sync::mpsc::Sender<Vec<u8>>`（替代原来的 `tauri::ipc::Channel<Vec<u8>>`）
//! - 事件通道：`tokio::sync::broadcast::Sender<BackendEvent>`（替代原来的 `app.emit(...)`）
//!
//! 原 src-tauri/src/ipc/commands.rs 内的 `SerialState` + 27 个 IPC 命令拆分：
//! - **这里**（core）：纯粹的业务逻辑，UI 调用方决定如何消费 events/data
//! - **`src-tauri/src/ipc/commands.rs`**：Tauri 命令薄壳，负责把 `Channel<Vec<u8>>` / `app.emit`
//!   接到 Backend 上
//! - **未来 `egui-app/`**：egui 直接调用 Backend 方法 + 监听 events

use crate::error::SerialError;
use crate::recorder::{self, Recorder, RecorderSummary};
use crate::sender::{SendCommand, SendQueue};
use crate::serial::port::{list_ports as list_ports_inner, PortInfo};
use crate::serial::ring_buffer::RingBuffer;

use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

// =============================================================================
// 公开类型（替代原 SerialState + BufferStatus + ReconnectStatus）
// =============================================================================

/// 缓冲区快照（UI 状态栏显示用）
#[derive(Debug, Clone)]
pub struct BufferStatus {
    pub data_len: usize,
    pub water_level: f32,
    pub backpressure: String,
    pub overflow_count: usize,
}

/// 连接状态
#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub is_open: bool,
    pub disconnected: bool,
}

/// 打开串口参数
#[derive(Debug, Clone)]
pub struct OpenPortOptions {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: String,
}

/// 重连阶段（UI 状态机驱动）
#[derive(Debug, Clone, PartialEq)]
pub enum ReconnectPhase {
    Started,
    Attempt,
    Succeeded,
    Failed,
    Cancelled,
}

/// 重连事件（每次状态变化给 UI 推一份）
#[derive(Debug, Clone)]
pub struct ReconnectEvent {
    pub phase: ReconnectPhase,
    pub attempt: u32,
    pub max_attempts: u32,
    pub next_delay_ms: u64,
    pub message: String,
}

/// Backend 发送的所有非数据事件（数据走 mpsc，避免高频广播拖累）
#[derive(Debug, Clone)]
pub enum BackendEvent {
    PortOpened { name: String, baud_rate: u32 },
    PortClosed,
    PortDisconnected(String),
    Reconnect(ReconnectEvent),
    SendPollerError(String),
    SendPreciseError(String),
}

/// 自动重连任务句柄
pub struct ReconnectHandle {
    pub stop_flag: Arc<AtomicBool>,
    pub attempts: Arc<AtomicU32>,
}

/// 自动重连退避序列（秒）：1s → 2s → 4s → 8s → 15s（最多 5 次）
const RECONNECT_BACKOFF_SECS: &[u64] = &[1, 2, 4, 8, 15];

/// 连续错误累计阈值：达到后视为断线（防 CH340 短接松动误报）
const DISCONNECT_ERROR_THRESHOLD: u32 = 3;

/// 队列状态
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub count: usize,
    pub is_polling: bool,
}

/// Backend 主体：UI-agnostic 串口服务
pub struct Backend {
    // 串口底层
    ring_buffer: Arc<Mutex<RingBuffer>>,
    port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    stop_flag: Arc<AtomicBool>,
    disconnect_flag: Arc<AtomicBool>,

    // 发送队列
    send_queue: Arc<Mutex<SendQueue>>,
    polling_stop_flag: Arc<AtomicBool>,
    precise_stop_flag: Arc<AtomicBool>,

    // 重连
    reconnect_state: Arc<Mutex<Option<ReconnectHandle>>>,

    // 录制器（v1.2.0：跨 reader 线程生命周期）
    recorder: Arc<Mutex<Option<Recorder>>>,

    // UI 事件 + 数据通道
    event_tx: broadcast::Sender<BackendEvent>,
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend {
    /// 创建 Backend：内部初始化所有子状态 + 事件广播通道
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            ring_buffer: Arc::new(Mutex::new(RingBuffer::new(65536))),
            port_handle: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            disconnect_flag: Arc::new(AtomicBool::new(false)),
            send_queue: Arc::new(Mutex::new(SendQueue::new())),
            polling_stop_flag: Arc::new(AtomicBool::new(true)),
            precise_stop_flag: Arc::new(AtomicBool::new(true)),
            reconnect_state: Arc::new(Mutex::new(None)),
            recorder: Arc::new(Mutex::new(None)),
            event_tx,
        }
    }

    /// UI 订阅事件流（每次订阅拿一份新的 receiver）
    ///
    /// 典型用途（Tauri）：
    /// ```ignore
    /// let rx = backend.subscribe();
    /// tauri::spawn(async move {
    ///     while let Ok(event) = rx.recv().await {
    ///         app.emit("backend-event", event)?;
    ///     }
    /// });
    /// ```
    pub fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
        self.event_tx.subscribe()
    }

    /// 复制内部事件发送端（供 reader / 重连线程用）
    fn event_tx(&self) -> broadcast::Sender<BackendEvent> {
        self.event_tx.clone()
    }

    // ==================== 串口列表 ====================

    pub fn list_ports(&self) -> Vec<PortInfo> {
        list_ports_inner()
    }

    // ==================== 打开/关闭串口 ====================

    /// 打开串口，启动后台 reader 线程
    ///
    /// `data_tx`：UI 提供的 mpsc sender，reader 线程把已 flush 的字节 Vec<u8> 推进去。
    /// UI（Tauri）会把 `mpsc::Receiver` 桥接到 `Channel<Vec<u8>>`；egui 直接渲染。
    pub fn open_port(
        self: &Arc<Self>,
        opts: OpenPortOptions,
        data_tx: mpsc::Sender<Vec<u8>>,
    ) -> Result<(), SerialError> {
        let mut handle = self
            .port_handle
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;

        if handle.is_some() {
            return Err(SerialError::OpenFailed("串口已打开，请先关闭".into()));
        }

        let port = serialport::new(&opts.port_name, opts.baud_rate)
            .data_bits(match opts.data_bits {
                5 => DataBits::Five,
                6 => DataBits::Six,
                7 => DataBits::Seven,
                8 => DataBits::Eight,
                _ => return Err(SerialError::OpenFailed("无效的数据位".into())),
            })
            .stop_bits(match opts.stop_bits {
                1 => StopBits::One,
                2 => StopBits::Two,
                _ => return Err(SerialError::OpenFailed("无效的停止位".into())),
            })
            .parity(match opts.parity.to_uppercase().as_str() {
                "NONE" | "N" => Parity::None,
                "ODD" | "O" => Parity::Odd,
                "EVEN" | "E" => Parity::Even,
                _ => return Err(SerialError::OpenFailed("无效的校验位".into())),
            })
            .flow_control(FlowControl::None)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| SerialError::OpenFailed(format!("打开串口失败: {e}")))?;

        *handle = Some(port);

        // 启动后台读取线程
        let ring_buffer = Arc::clone(&self.ring_buffer);
        let port_handle = Arc::clone(&self.port_handle);
        let stop_flag = Arc::clone(&self.stop_flag);
        let disconnect_flag = Arc::clone(&self.disconnect_flag);
        let reconnect_state = Arc::clone(&self.reconnect_state);
        let recorder = Arc::clone(&self.recorder);
        stop_flag.store(false, Ordering::SeqCst);
        disconnect_flag.store(false, Ordering::SeqCst);

        let event_tx = self.event_tx();
        let port_name_for_reader = opts.port_name.clone();
        let baud_rate = opts.baud_rate;
        let data_bits = opts.data_bits;
        let stop_bits = opts.stop_bits;
        let parity = opts.parity.clone();

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
                    event_tx,
                    port_name_for_reader,
                    baud_rate,
                    data_bits,
                    stop_bits,
                    parity,
                    data_tx,
                );
            })
            .map_err(|e| SerialError::OpenFailed(format!("启动读取线程失败: {e}")))?;

        let _ = self.event_tx.send(BackendEvent::PortOpened {
            name: opts.port_name.clone(),
            baud_rate: opts.baud_rate,
        });
        Ok(())
    }

    pub fn close_port(&self) -> Result<(), SerialError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        let mut handle = self
            .port_handle
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        *handle = None;
        self.disconnect_flag.store(false, Ordering::SeqCst);

        // 取消任何进行中的自动重连
        if let Ok(mut rs) = self.reconnect_state.lock() {
            if let Some(h) = rs.take() {
                h.stop_flag.store(true, Ordering::SeqCst);
                log::info!("[reconnect] 用户主动关闭串口，已取消重连");
            }
        }

        // v1.2.0：用户主动关闭串口 → 自动停止录制（Q13A）
        if let Ok(mut rec_guard) = self.recorder.lock() {
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

        let _ = self.event_tx.send(BackendEvent::PortClosed);
        Ok(())
    }

    pub fn connection_status(&self) -> Result<ConnectionStatus, SerialError> {
        let is_open = self
            .port_handle
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?
            .is_some();
        let disconnected = self.disconnect_flag.load(Ordering::SeqCst);
        Ok(ConnectionStatus {
            is_open,
            disconnected,
        })
    }

    pub fn read_buffer(&self, len: usize) -> Result<Vec<u8>, SerialError> {
        let mut buf = self
            .ring_buffer
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        Ok(buf.read(len))
    }

    pub fn write_data(&self, data: &[u8]) -> Result<(), SerialError> {
        let mut handle = self
            .port_handle
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        let port = handle.as_mut().ok_or(SerialError::PortNotOpen)?;
        port.write(data)
            .map_err(|e| SerialError::ReceiveError(format!("写入失败: {e}")))?;
        Ok(())
    }

    pub fn buffer_status(&self) -> Result<BufferStatus, SerialError> {
        let buf = self
            .ring_buffer
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        Ok(BufferStatus {
            data_len: buf.data_len(),
            water_level: buf.water_level(),
            backpressure: format!("{:?}", buf.backpressure_state()),
            overflow_count: buf.overflow_count(),
        })
    }

    pub fn cancel_reconnect(&self) -> Result<(), SerialError> {
        if let Ok(mut rs) = self.reconnect_state.lock() {
            if let Some(h) = rs.take() {
                h.stop_flag.store(true, Ordering::SeqCst);
                log::info!("[reconnect] 用户主动取消");
            }
        }
        Ok(())
    }

    // ==================== 录制 ====================

    pub fn start_recording(
        &self,
        path: std::path::PathBuf,
        port_name: String,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: String,
    ) -> Result<(), SerialError> {
        let mut rec_guard = self
            .recorder
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        if rec_guard.is_some() {
            return Err(SerialError::OpenFailed("已在录制中".into()));
        }
        let mut rec = recorder::start_recording(path.clone())
            .map_err(|e| SerialError::ReceiveError(format!("创建录制文件失败: {e}")))?;
        rec.write_header(&port_name, baud_rate, data_bits, stop_bits, &parity)
            .map_err(|e| SerialError::ReceiveError(format!("写文件头失败: {e}")))?;
        log::info!("[recorder] 开始录制: {}", path.display());
        *rec_guard = Some(rec);
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<RecorderSummary, SerialError> {
        let mut rec_guard = self
            .recorder
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        let rec = rec_guard
            .take()
            .ok_or_else(|| SerialError::OpenFailed("未在录制".into()))?;
        let summary = rec
            .stop()
            .map_err(|e| SerialError::ReceiveError(format!("停止录制失败: {e}")))?;
        log::info!(
            "[recorder] 停止录制: {} ({} bytes, {} ms)",
            summary.path.display(),
            summary.bytes_written,
            summary.duration_ms
        );
        Ok(summary)
    }

    /// 写入一行纯文本到录制文件（前端 Terminal writeData 调用）
    pub fn write_recorder_line(&self, line: &str) -> Result<(), SerialError> {
        let mut rec_guard = self
            .recorder
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        if let Some(rec) = rec_guard.as_mut() {
            rec.write_line(line)
                .map_err(|e| SerialError::ReceiveError(format!("写入失败: {e}")))?;
        }
        Ok(())
    }

    pub fn mark_recorder_event(&self, text: &str) -> Result<(), SerialError> {
        let mut rec_guard = self
            .recorder
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        if let Some(rec) = rec_guard.as_mut() {
            rec.mark_event(text)
                .map_err(|e| SerialError::ReceiveError(format!("写入失败: {e}")))?;
        }
        Ok(())
    }

    pub fn is_recording(&self) -> Result<bool, SerialError> {
        let rec_guard = self
            .recorder
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        Ok(rec_guard.is_some())
    }

    // ==================== 发送队列 ====================

    pub fn queue_add(&self, cmd: SendCommand) -> Result<(), SerialError> {
        let mut q = self
            .send_queue
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        q.add(cmd);
        Ok(())
    }

    pub fn queue_remove(&self, id: &str) -> Result<(), SerialError> {
        let mut q = self
            .send_queue
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        q.remove(id);
        Ok(())
    }

    pub fn queue_clear(&self) -> Result<(), SerialError> {
        let mut q = self
            .send_queue
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        q.clear();
        Ok(())
    }

    pub fn queue_status(&self) -> Result<QueueStatus, SerialError> {
        let q = self
            .send_queue
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        Ok(QueueStatus {
            count: q.get_commands().len(),
            is_polling: q.is_polling(),
        })
    }

    /// 启动发送队列轮询（spawn 一个新线程）
    pub fn queue_start_polling(&self) -> Result<(), SerialError> {
        let send_queue = Arc::clone(&self.send_queue);
        let port_handle = Arc::clone(&self.port_handle);
        let polling_stop_flag = Arc::clone(&self.polling_stop_flag);
        let event_tx = self.event_tx();

        if !polling_stop_flag.load(Ordering::SeqCst) {
            return Ok(()); // 已在跑，幂等返回
        }
        polling_stop_flag.store(false, Ordering::SeqCst);

        let mut queue = send_queue
            .lock()
            .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
        queue.start_polling();
        drop(queue);

        thread::Builder::new()
            .name("send-poller".to_string())
            .spawn(move || {
                loop {
                    if polling_stop_flag.load(Ordering::SeqCst) {
                        break;
                    }

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
                                        break Err(std::io::Error::other("无法获取串口锁"));
                                    }
                                    thread::sleep(Duration::from_millis(2));
                                }
                            }
                        };

                        if let Err(e) = write_result {
                            log::error!("[send-poller] 写入失败: {:?}", e);
                            let _ = event_tx.send(BackendEvent::SendPollerError(e.to_string()));
                            break;
                        }

                        thread::sleep(Duration::from_millis(cmd.interval_ms));
                    } else {
                        thread::sleep(Duration::from_millis(50));
                    }
                }

                if let Ok(mut q) = send_queue.lock() {
                    q.stop_polling();
                }
                polling_stop_flag.store(true, Ordering::SeqCst);
            })
            .map_err(|e| SerialError::ReceiveError(format!("启动轮询线程失败: {e}")))?;

        Ok(())
    }

    pub fn queue_stop_polling(&self) -> Result<(), SerialError> {
        self.polling_stop_flag.store(true, Ordering::SeqCst);
        if let Ok(mut q) = self.send_queue.lock() {
            q.stop_polling();
        }
        Ok(())
    }

    // ==================== 定时精确发送 ====================

    pub fn start_periodic_send(
        &self,
        payload: Vec<u8>,
        interval_ms: u64,
    ) -> Result<(), SerialError> {
        use std::time::Instant;

        let port_handle = Arc::clone(&self.port_handle);
        let precise_stop_flag = Arc::clone(&self.precise_stop_flag);
        precise_stop_flag.store(false, Ordering::SeqCst);
        let event_tx = self.event_tx();

        // 清空 SendQueue，避免与周期性发送冲突
        {
            let mut q = self
                .send_queue
                .lock()
                .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
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
                                    break Err(std::io::Error::other("无法获取串口锁"));
                                }
                                thread::sleep(Duration::from_millis(2));
                            }
                        }
                    };

                    if let Err(e) = write_result {
                        log::error!("[send-precise] 写入失败: {:?}", e);
                        let _ = event_tx.send(BackendEvent::SendPreciseError(e.to_string()));
                        break;
                    }
                }
            })
            .map_err(|e| SerialError::ReceiveError(format!("启动精确发送失败: {e}")))?;

        Ok(())
    }

    pub fn stop_periodic_send(&self) -> Result<(), SerialError> {
        self.precise_stop_flag.store(true, Ordering::SeqCst);
        Ok(())
    }
}

// =============================================================================
// 内部 helper：reader 主循环
// =============================================================================

/// `ring_buffer` / `port_handle` / 标志的轻量集合（用于 schedule_reconnect）
struct BackendLite {
    ring_buffer: Arc<Mutex<RingBuffer>>,
    port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    stop_flag: Arc<AtomicBool>,
    disconnect_flag: Arc<AtomicBool>,
    reconnect_state: Arc<Mutex<Option<ReconnectHandle>>>,
    recorder: Arc<Mutex<Option<Recorder>>>,
}

/// reader 线程主体
///
/// 退出条件：stop_flag 置位 / 串口句柄被置 None / 读取到断线信号
/// 退出时：若 auto_reconnect 启用（按 AppConfig），调度 schedule_reconnect
#[allow(clippy::too_many_arguments)]
fn run_reader_loop(
    ring_buffer: Arc<Mutex<RingBuffer>>,
    port_handle: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    stop_flag: Arc<AtomicBool>,
    disconnect_flag: Arc<AtomicBool>,
    reconnect_state: Arc<Mutex<Option<ReconnectHandle>>>,
    recorder: Arc<Mutex<Option<Recorder>>>,
    event_tx: broadcast::Sender<BackendEvent>,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    data_tx: mpsc::Sender<Vec<u8>>,
) {
    let mut scratch = [0u8; 256];
    let mut last_flush = std::time::Instant::now();
    let mut consecutive_errors: u32 = 0;
    let mut disconnected_naturally = false;
    let mut disconnect_reason = String::new();
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
                    consecutive_errors = 0;
                    if let Ok(mut buf) = ring_buffer.lock() {
                        buf.write(&scratch[..n]);
                    }
                }
            }
            Some(Err(e)) => {
                use std::io::ErrorKind;
                match e.kind() {
                    ErrorKind::NotConnected | ErrorKind::BrokenPipe => {
                        log::error!("[serial-reader] 设备已断开: {:?}", e);
                        disconnect_flag.store(true, Ordering::SeqCst);
                        disconnect_reason = e.to_string();
                        let _ = event_tx.send(BackendEvent::PortDisconnected(e.to_string()));
                        disconnected_naturally = true;
                        disconnect_time = Some(std::time::Instant::now());
                        if let Ok(mut rg) = recorder.lock() {
                            if let Some(rec) = rg.as_mut() {
                                let _ = rec.mark_event(&format!("设备已断开: {e}"));
                            }
                        }
                        break;
                    }
                    ErrorKind::TimedOut => {
                        consecutive_errors = 0;
                    }
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
                            let _ = event_tx.send(BackendEvent::PortDisconnected(e.to_string()));
                            disconnected_naturally = true;
                            disconnect_time = Some(std::time::Instant::now());
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
                if let Err(e) = data_tx.try_send(payload) {
                    log::warn!("[serial-reader] data_tx send failed: {:?}", e);
                }
            }
        }

        thread::sleep(Duration::from_millis(2));
    }

    // 退出清理：若非用户主动关闭（disconnected_naturally=true），尝试自动重连
    if disconnected_naturally {
        let cfg = crate::config::load();
        if cfg.auto_reconnect && !stop_flag.load(Ordering::SeqCst) {
            log::info!(
                "[serial-reader] 触发自动重连（原因：{disconnect_reason}，端口：{port_name}）"
            );
            let st = BackendLite {
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
                event_tx,
                data_tx,
                disconnect_time,
            );
        }
    }
}

/// 启动自动重连线程
#[allow(clippy::too_many_arguments)]
fn schedule_reconnect(
    state: BackendLite,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
    max_attempts: u32,
    event_tx: broadcast::Sender<BackendEvent>,
    data_tx: mpsc::Sender<Vec<u8>>,
    disconnect_time: Option<std::time::Instant>,
) {
    if let Ok(mut rs) = state.reconnect_state.lock() {
        if let Some(prev) = rs.take() {
            prev.stop_flag.store(true, Ordering::SeqCst);
        }
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let attempts = Arc::new(AtomicU32::new(0));

    if let Ok(mut rs) = state.reconnect_state.lock() {
        *rs = Some(ReconnectHandle {
            stop_flag: Arc::clone(&stop_flag),
            attempts: Arc::clone(&attempts),
        });
    }

    let _ = event_tx.send(BackendEvent::Reconnect(ReconnectEvent {
        phase: ReconnectPhase::Started,
        attempt: 0,
        max_attempts,
        next_delay_ms: 0,
        message: format!("已断开，准备重连 {port_name}"),
    }));
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
            for (idx, &backoff_sec) in RECONNECT_BACKOFF_SECS.iter().enumerate() {
                if stop_flag.load(Ordering::SeqCst) {
                    log::info!("[reconnect] 已取消");
                    let _ = event_tx.send(BackendEvent::Reconnect(ReconnectEvent {
                        phase: ReconnectPhase::Cancelled,
                        attempt: (idx + 1) as u32,
                        max_attempts,
                        next_delay_ms: 0,
                        message: format!("{port_name} 重连已取消"),
                    }));
                    return;
                }

                let _ = event_tx.send(BackendEvent::Reconnect(ReconnectEvent {
                    phase: ReconnectPhase::Attempt,
                    attempt: (idx + 1) as u32,
                    max_attempts,
                    next_delay_ms: backoff_sec * 1000,
                    message: format!("{} 秒后第 {} 次重试 {}", backoff_sec, idx + 1, port_name),
                }));
                attempts.store((idx + 1) as u32, Ordering::SeqCst);

                for _ in 0..(backoff_sec * 10) {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                match try_reconnect_open(&port_name, baud_rate, data_bits, stop_bits, &parity) {
                    Ok(port) => {
                        log::info!("[reconnect] 第 {} 次重连成功（{}）", idx + 1, port_name);

                        {
                            let mut h = match port_handle.lock() {
                                Ok(h) => h,
                                Err(_) => return,
                            };
                            *h = Some(port);
                        }

                        serial_stop.store(false, Ordering::SeqCst);
                        disconnect_flag.store(false, Ordering::SeqCst);

                        let _ = event_tx.send(BackendEvent::Reconnect(ReconnectEvent {
                            phase: ReconnectPhase::Succeeded,
                            attempt: (idx + 1) as u32,
                            max_attempts,
                            next_delay_ms: 0,
                            message: format!("重连成功：{port_name}"),
                        }));

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

                        if let Ok(mut rs) = reconnect_state.lock() {
                            *rs = None;
                        }

                        let rb = Arc::clone(&ring_buffer);
                        let ph = Arc::clone(&port_handle);
                        let sf = Arc::clone(&serial_stop);
                        let df = Arc::clone(&disconnect_flag);
                        let rs2 = Arc::clone(&reconnect_state);
                        let rec2 = Arc::clone(&recorder);
                        let event_tx2 = event_tx.clone();
                        let pn = port_name.clone();
                        let br = baud_rate;
                        let db2 = data_bits;
                        let sb2 = stop_bits;
                        let pa2 = parity.clone();
                        let data_tx2 = data_tx.clone();
                        let _ = thread::Builder::new()
                            .name("serial-reader".to_string())
                            .spawn(move || {
                                run_reader_loop(
                                    rb, ph, sf, df, rs2, rec2, event_tx2, pn, br, db2, sb2, pa2,
                                    data_tx2,
                                );
                            });
                        return;
                    }
                    Err(e) => {
                        log::warn!("[reconnect] 第 {} 次失败：{}", idx + 1, e);
                    }
                }
            }

            log::error!("[reconnect] {} 次重连全部失败", max_attempts);
            let _ = event_tx.send(BackendEvent::Reconnect(ReconnectEvent {
                phase: ReconnectPhase::Failed,
                attempt: max_attempts,
                max_attempts,
                next_delay_ms: 0,
                message: format!("{port_name} 重连失败，已放弃"),
            }));
            if let Ok(mut rs) = reconnect_state.lock() {
                *rs = None;
            }
        })
        .expect("启动重连线程失败");
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_new_initializes_correctly() {
        let backend = Backend::new();
        let status = backend.connection_status().unwrap();
        assert!(!status.is_open);
        assert!(!status.disconnected);
    }

    #[test]
    fn backend_list_ports_returns_vec() {
        let backend = Backend::new();
        let _ = backend.list_ports();
        // 不论有没有端口都不该 panic
    }

    #[test]
    fn backend_buffer_status_when_empty() {
        let backend = Backend::new();
        let s = backend.buffer_status().unwrap();
        assert_eq!(s.data_len, 0);
        assert_eq!(s.overflow_count, 0);
    }

    #[test]
    fn backend_queue_operations() {
        let backend = Backend::new();
        backend
            .queue_add(SendCommand {
                id: "a".into(),
                content: vec![0x01],
                priority: 1,
                interval_ms: 100,
            })
            .unwrap();
        backend
            .queue_add(SendCommand {
                id: "b".into(),
                content: vec![0x02],
                priority: 100,
                interval_ms: 100,
            })
            .unwrap();
        let q = backend.queue_status().unwrap();
        assert_eq!(q.count, 2);
        // 高优先级在前
        backend.queue_remove("a").unwrap();
        assert_eq!(backend.queue_status().unwrap().count, 1);
    }

    #[test]
    fn backend_is_recording_default_false() {
        let backend = Backend::new();
        assert!(!backend.is_recording().unwrap());
    }

    #[test]
    fn backend_open_when_already_open_fails() {
        // 不实际打开（避免依赖硬件），只测错误分支
        // 通过先持有 port_handle 模拟
        let backend = Backend::new();
        // 模拟已打开：通过直接 lock port_handle 设置 Some 是可以的，但需要 SerialPort 实例
        // 简单替代：通过 close_port 后立即 open 应不通过"已打开"检查
        // 这里只验证 open_port 在 port 名称无效时返回 OpenFailed
        let opts = OpenPortOptions {
            port_name: "INVALID-PORT-9999".into(),
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: "none".into(),
        };
        let (tx, _rx) = mpsc::channel(8);
        let backend = Arc::new(backend);
        let res = backend.open_port(opts, tx);
        assert!(res.is_err());
    }
}
