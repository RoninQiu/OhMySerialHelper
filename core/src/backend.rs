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
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
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
    PortOpened {
        name: String,
        baud_rate: u32,
    },
    PortClosed,
    PortDisconnected(String),
    Reconnect(ReconnectEvent),
    SendPollerError(String),
    SendPreciseError(String),
    /// core 直接写口的两条路径（队列轮询 / 周期发送）写入成功。
    /// 面板触发的普通发送**不走这里**。
    TxEcho(Vec<u8>),
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

/// 串口读超时。reader 跨阻塞读**持锁**，这个值直接决定写线程能拿到锁的概率。
const READ_TIMEOUT: Duration = Duration::from_millis(10);

/// 一次写入抢串口锁的总预算。超了不报错，只算本 tick 争用失败。
const WRITE_LOCK_BUDGET: Duration = Duration::from_millis(500);
/// 抢锁失败后的重试间隔。
const WRITE_LOCK_RETRY: Duration = Duration::from_millis(2);
/// 争用跳过后退避多久再试同一条命令。
const CONTENTION_BACKOFF: Duration = Duration::from_millis(10);
/// 队列为空时的轮询间隔。
const POLL_IDLE_TICK: Duration = Duration::from_millis(50);

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
    /// 每次 start/stop 自增。线程各自记住启动时的代号，对不上就退出。
    polling_generation: Arc<AtomicU64>,
    precise_generation: Arc<AtomicU64>,
    /// 此刻**真正在跑**的 poller 线程数。
    ///
    /// 不是「该不该跑」的标志位，而是由每个线程在退出时自己 `fetch_sub` 归位的
    /// 存活计数。停止时**不能**由 stop 去清它——那会立刻变成「false」而线程
    /// 其实还躺在 sleep 里，测出来的「停止生效」是假的。
    /// 停止后它最多在一个 sleep 分片（≤200ms）内归零，这才是真判据。
    polling_threads: Arc<AtomicU32>,
    precise_threads: Arc<AtomicU32>,
    /// poller 与 precise 的写入字节数（不含宿主自己的发送计数）。
    timed_tx_bytes: Arc<AtomicU64>,

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
            polling_generation: Arc::new(AtomicU64::new(0)),
            precise_generation: Arc::new(AtomicU64::new(0)),
            polling_threads: Arc::new(AtomicU32::new(0)),
            precise_threads: Arc::new(AtomicU32::new(0)),
            timed_tx_bytes: Arc::new(AtomicU64::new(0)),
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
            .timeout(READ_TIMEOUT)
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
    ///
    /// 幂等：已在跑就直接返回。停止后立刻重来会拿到新的代号，
    /// 旧线程发现代号对不上自行退出——**不会**出现双 poller。
    pub fn queue_start_polling(&self) -> Result<(), SerialError> {
        {
            let q = self
                .send_queue
                .lock()
                .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
            if q.is_polling() && self.polling_threads.load(Ordering::SeqCst) > 0 {
                return Ok(()); // 已在跑
            }
        }

        let my_gen = self.polling_generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut q = self
                .send_queue
                .lock()
                .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
            q.start_polling();
        }

        let send_queue = Arc::clone(&self.send_queue);
        let port_handle = Arc::clone(&self.port_handle);
        let polling_generation = Arc::clone(&self.polling_generation);
        let polling_threads = Arc::clone(&self.polling_threads);
        let timed_tx_bytes = Arc::clone(&self.timed_tx_bytes);
        let event_tx = self.event_tx();
        polling_threads.fetch_add(1, Ordering::SeqCst);

        let spawn = thread::Builder::new()
            .name("send-poller".to_string())
            .spawn(move || {
                loop {
                    if polling_generation.load(Ordering::SeqCst) != my_gen {
                        break;
                    }

                    let next = {
                        let Ok(mut q) = send_queue.lock() else { break };
                        if !q.is_polling() {
                            break;
                        }
                        q.next_command().cloned()
                    };

                    let Some(cmd) = next else {
                        if !sleep_interruptible(&polling_generation, my_gen, POLL_IDLE_TICK) {
                            break;
                        }
                        continue;
                    };

                    match write_with_retry(&port_handle, &cmd.content, WRITE_LOCK_BUDGET) {
                        WriteOutcome::Sent(n) => {
                            timed_tx_bytes.fetch_add(n as u64, Ordering::Relaxed);
                            let _ = event_tx.send(BackendEvent::TxEcho(cmd.content.clone()));
                            if !sleep_interruptible(
                                &polling_generation,
                                my_gen,
                                Duration::from_millis(cmd.interval_ms),
                            ) {
                                break;
                            }
                        }
                        WriteOutcome::Contended => {
                            // 抢锁失败，但 next_command 已经把游标推进了。
                            // 退回一格，下个 tick 重试**同一条**命令。
                            if let Ok(mut q) = send_queue.lock() {
                                q.rewind();
                            }
                            if !sleep_interruptible(&polling_generation, my_gen, CONTENTION_BACKOFF)
                            {
                                break;
                            }
                        }
                        WriteOutcome::Failed(e) => {
                            log::error!("[send-poller] 写入失败: {:?}", e);
                            let _ = event_tx.send(BackendEvent::SendPollerError(e.to_string()));
                            break;
                        }
                    }
                }

                // 只有代号仍是自己的那个线程才能收尾，否则会误停新线程的轮询。
                if polling_generation.load(Ordering::SeqCst) == my_gen {
                    if let Ok(mut q) = send_queue.lock() {
                        q.stop_polling();
                    }
                }
                // 存活计数无条件归位：这是**这个线程自己的**份额。
                // 不按代号 gating——旧线程晚退一步不该把新线程的份额也减掉。
                polling_threads.fetch_sub(1, Ordering::SeqCst);
            });

        if let Err(e) = spawn {
            // spawn 失败要把计数还原，否则之后再也起不来。
            self.polling_threads.fetch_sub(1, Ordering::SeqCst);
            if let Ok(mut q) = self.send_queue.lock() {
                q.stop_polling();
            }
            return Err(SerialError::ReceiveError(format!("启动轮询线程失败: {e}")));
        }

        Ok(())
    }

    pub fn queue_stop_polling(&self) -> Result<(), SerialError> {
        if let Ok(mut q) = self.send_queue.lock() {
            q.stop_polling();
        }
        // 这里**不**动 polling_threads：退出由线程自己归位。
        // 提前清零会让「停止生效」看起来是瞬时的，而线程其实还躺在 sleep 里——
        // 那正是本轮要修的缺陷本身，判据不能建立在这个假象上。
        self.polling_generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// 此刻真正在跑的 poller 线程数（诊断与测试用）。
    ///
    /// 停止之后它要等旧线程自己醒来（≤200ms）才归零。用来判「线程真的退出了吗」——
    /// `queue_status().is_polling` 做不到：那个位是 stop 同步置的，
    /// 旧实现里它同样会立刻变 false。
    pub fn polling_thread_count(&self) -> u32 {
        self.polling_threads.load(Ordering::SeqCst)
    }

    /// 便捷判据：还有 poller 线程在跑吗。
    pub fn is_polling_thread_alive(&self) -> bool {
        self.polling_thread_count() > 0
    }

    // ==================== 定时精确发送 ====================

    pub fn start_periodic_send(
        &self,
        payload: Vec<u8>,
        interval_ms: u64,
    ) -> Result<(), SerialError> {
        if self.precise_threads.load(Ordering::SeqCst) > 0 {
            return Ok(()); // 已在跑
        }
        let my_gen = self.precise_generation.fetch_add(1, Ordering::SeqCst) + 1;

        // 周期发送与队列轮询互斥。原先靠 `clear()` 顺带清 `is_polling` 达成，
        // 现在 `clear()` 不再碰轮询状态了，得显式停。
        let _ = self.queue_stop_polling();

        // 清空待发队列，避免与周期性发送冲突
        {
            let mut q = self
                .send_queue
                .lock()
                .map_err(|e| SerialError::ReceiveError(format!("锁失败: {e}")))?;
            q.clear();
        }

        let port_handle = Arc::clone(&self.port_handle);
        let precise_generation = Arc::clone(&self.precise_generation);
        let precise_threads = Arc::clone(&self.precise_threads);
        precise_threads.fetch_add(1, Ordering::SeqCst);
        let timed_tx_bytes = Arc::clone(&self.timed_tx_bytes);
        let event_tx = self.event_tx();

        let spawn = thread::Builder::new()
            .name("send-precise".to_string())
            .spawn(move || {
                let interval_dur = Duration::from_millis(interval_ms.max(1));
                let mut next_tick = Instant::now() + interval_dur;

                loop {
                    if precise_generation.load(Ordering::SeqCst) != my_gen {
                        break;
                    }

                    let now = Instant::now();
                    if now < next_tick
                        && !sleep_interruptible(&precise_generation, my_gen, next_tick - now)
                    {
                        break;
                    }
                    next_tick += interval_dur;

                    match write_with_retry(&port_handle, &payload, WRITE_LOCK_BUDGET) {
                        WriteOutcome::Sent(n) => {
                            timed_tx_bytes.fetch_add(n as u64, Ordering::Relaxed);
                            let _ = event_tx.send(BackendEvent::TxEcho(payload.clone()));
                        }
                        WriteOutcome::Contended => {
                            // 争用不是错误：跳过本 tick，线程活着等下个周期。
                            log::debug!("[send-precise] 本周期未抢到串口锁，跳过");
                        }
                        WriteOutcome::Failed(e) => {
                            log::error!("[send-precise] 写入失败: {:?}", e);
                            let _ = event_tx.send(BackendEvent::SendPreciseError(e.to_string()));
                            break;
                        }
                    }
                }

                // 存活计数无条件归位：这是这个线程自己的份额（理由同 poller）
                precise_threads.fetch_sub(1, Ordering::SeqCst);
            });

        if let Err(e) = spawn {
            self.precise_threads.fetch_sub(1, Ordering::SeqCst);
            return Err(SerialError::ReceiveError(format!("启动精确发送失败: {e}")));
        }

        Ok(())
    }

    pub fn stop_periodic_send(&self) -> Result<(), SerialError> {
        // 同 queue_stop_polling：存活计数交给线程自己归位
        self.precise_generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// 此刻真正在跑的周期发送线程数（诊断与测试用）。
    pub fn periodic_thread_count(&self) -> u32 {
        self.precise_threads.load(Ordering::SeqCst)
    }

    /// 便捷判据：还有周期发送线程在跑吗。
    pub fn is_periodic_thread_alive(&self) -> bool {
        self.periodic_thread_count() > 0
    }

    /// poller 与 precise 已写入的字节数。
    ///
    /// 与宿主自己的「用户手动发送计数」天然不相交，接收方直接并进去即可。
    pub fn timed_tx_bytes(&self) -> u64 {
        self.timed_tx_bytes.load(Ordering::Relaxed)
    }
}

// =============================================================================
// 内部 helper：写入与可中断等待
// =============================================================================

/// 一次写入的结果。**争用与失败必须分开**——旧代码把「抢不到锁」也
/// 构造成 IO 错误再 `break` 掉线程，等于把临时争用当成致命错误，
/// 这是 poller 静默死掉的直接原因。
#[derive(Debug)]
enum WriteOutcome {
    /// 写入成功，值为字节数。
    Sent(usize),
    /// 预算内没抢到锁：只跳过本 tick，线程继续活着。
    Contended,
    /// 真正的写失败（串口未打开 / IO 错误）：终止线程。
    Failed(std::io::Error),
}

/// 抢锁写入。`budget` 内抢不到锁就返回 `Contended`，不构造假错误。
fn write_with_retry(
    port_handle: &Mutex<Option<Box<dyn SerialPort>>>,
    payload: &[u8],
    budget: Duration,
) -> WriteOutcome {
    let deadline = Instant::now() + budget;
    loop {
        match port_handle.try_lock() {
            Ok(mut guard) => {
                return match guard.as_mut() {
                    Some(port) => match port.write_all(payload) {
                        Ok(()) => WriteOutcome::Sent(payload.len()),
                        Err(e) => WriteOutcome::Failed(e),
                    },
                    None => WriteOutcome::Failed(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "串口未打开",
                    )),
                };
            }
            Err(_) => {
                if Instant::now() >= deadline {
                    return WriteOutcome::Contended;
                }
                thread::sleep(WRITE_LOCK_RETRY);
            }
        }
    }
}

/// 分片长度：interval 的 1/20，钳在 20–200ms。
fn wait_slice(total: Duration) -> Duration {
    (total / 20).clamp(Duration::from_millis(20), Duration::from_millis(200))
}

/// 分片睡眠，每片之间检查代号是否被换掉。
///
/// 返回 `true` = 睡满了 `total`；`false` = 被 stop/start 打断。
/// 这是「点了停止最长 60 秒才生效」的解药。
fn sleep_interruptible(gen: &AtomicU64, my_gen: u64, total: Duration) -> bool {
    let slice = wait_slice(total);
    let deadline = Instant::now() + total;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return true;
        }
        if gen.load(Ordering::SeqCst) != my_gen {
            return false;
        }
        thread::sleep(slice.min(deadline.saturating_duration_since(now)));
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

    // ===== 写入与可中断等待 =====

    /// 串口未打开时是 Failed，不是 Contended——两者混淆正是旧的静默死掉。
    #[test]
    fn write_with_retry_on_closed_port_is_failed() {
        let handle: Mutex<Option<Box<dyn SerialPort>>> = Mutex::new(None);
        match write_with_retry(&handle, b"hi", Duration::from_millis(20)) {
            WriteOutcome::Failed(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotConnected),
            WriteOutcome::Contended => panic!("串口没开却报争用"),
            WriteOutcome::Sent(n) => panic!("串口没开却写成功了 {n}"),
        }
    }

    /// 锁被别人长期占着时，预算耗尽后是 Contended（而不是假 IO 错误）。
    #[test]
    fn write_with_retry_gives_up_as_contended_after_budget() {
        let handle: Mutex<Option<Box<dyn SerialPort>>> = Mutex::new(None);
        let _held = handle.lock().unwrap(); // 永久持锁
        let start = Instant::now();
        let out = write_with_retry(&handle, b"hi", Duration::from_millis(60));
        let elapsed = start.elapsed();
        assert!(matches!(out, WriteOutcome::Contended), "应是争用而非失败");
        assert!(
            elapsed >= Duration::from_millis(55),
            "预算 {elapsed:?} 太短"
        );
        assert!(
            elapsed < Duration::from_millis(2000),
            "预算 {elapsed:?} 失控"
        );
    }

    #[test]
    fn wait_slice_is_clamped_to_20_200ms() {
        // interval 的 1/20，下限 20ms
        assert_eq!(
            wait_slice(Duration::from_millis(50)),
            Duration::from_millis(20)
        );
        assert_eq!(
            wait_slice(Duration::from_millis(400)),
            Duration::from_millis(20)
        );
        assert_eq!(
            wait_slice(Duration::from_millis(1000)),
            Duration::from_millis(50)
        );
        // 上限 200ms
        assert_eq!(
            wait_slice(Duration::from_millis(60_000)),
            Duration::from_millis(200)
        );
    }

    /// 不换代号就睡满，返回 true。
    #[test]
    fn sleep_interruptible_completes_when_generation_unchanged() {
        let gen = AtomicU64::new(7);
        let start = Instant::now();
        assert!(sleep_interruptible(&gen, 7, Duration::from_millis(100)));
        assert!(start.elapsed() >= Duration::from_millis(90));
    }

    /// 换了代号就提前返回 false——「停止」在 200ms 内生效，而不是 60 秒。
    #[test]
    fn sleep_interruptible_returns_early_when_generation_changes() {
        let gen = Arc::new(AtomicU64::new(1));
        let g = Arc::clone(&gen);
        let bump = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            g.store(2, Ordering::SeqCst);
        });
        let start = Instant::now();
        let completed = sleep_interruptible(&gen, 1, Duration::from_secs(60));
        let elapsed = start.elapsed();
        bump.join().unwrap();
        assert!(!completed, "代号已变，不该报告睡满");
        assert!(
            elapsed < Duration::from_millis(1000),
            "唤醒太慢: {elapsed:?}"
        );
    }

    // ===== 生命周期：代际号 =====

    /// 停止后立刻开始：队列停在非轮询态、running 归位，
    /// 下一次 start 一定能重新拉起（旧的「静默早退」就死在这）。
    #[test]
    fn stop_then_start_polling_is_not_silently_ignored() {
        let backend = Backend::new();
        backend.queue_start_polling().unwrap();
        backend.queue_stop_polling().unwrap();
        backend.queue_start_polling().unwrap();
        let status = backend.queue_status().unwrap();
        assert!(status.is_polling, "停止后重新开始必须真的开始");
        backend.queue_stop_polling().unwrap();
    }

    /// 连按两次开始是幂等的，不会起两个线程。
    #[test]
    fn start_polling_twice_is_idempotent() {
        let backend = Backend::new();
        let before = backend.polling_generation.load(Ordering::SeqCst);
        backend.queue_start_polling().unwrap();
        let after_first = backend.polling_generation.load(Ordering::SeqCst);
        backend.queue_start_polling().unwrap();
        let after_second = backend.polling_generation.load(Ordering::SeqCst);
        assert_eq!(after_first, after_second, "重复 start 不该再 spawn");
        assert_eq!(after_first, before + 1);
        backend.queue_stop_polling().unwrap();
    }

    #[test]
    fn stop_polling_bumps_generation() {
        let backend = Backend::new();
        let before = backend.polling_generation.load(Ordering::SeqCst);
        backend.queue_stop_polling().unwrap();
        assert_eq!(
            backend.polling_generation.load(Ordering::SeqCst),
            before + 1
        );
    }

    #[test]
    fn stop_periodic_send_bumps_generation() {
        let backend = Backend::new();
        let before = backend.precise_generation.load(Ordering::SeqCst);
        backend.stop_periodic_send().unwrap();
        assert_eq!(
            backend.precise_generation.load(Ordering::SeqCst),
            before + 1
        );
    }

    /// 空队列时 poller 应保持运行（不会去写口，所以不会因写失败而退出）。
    #[test]
    fn poller_keeps_running_on_empty_queue() {
        let backend = Backend::new();
        backend.queue_start_polling().unwrap();
        thread::sleep(Duration::from_millis(200));
        assert!(backend.is_polling_thread_alive(), "空队列时轮询应保持运行");
        backend.queue_stop_polling().unwrap();
    }

    /// 停止后存活计数必须**由线程自己**归零，而不是 stop 顺手清。
    ///
    /// 这条是判据本身：如果 stop 直接清零，「停止生效耗时」会量出 0，
    /// 而线程其实还躺在 sleep 里——本轮要修的缺陷就被测试掩盖了。
    #[test]
    fn stop_does_not_falsely_report_thread_dead() {
        let backend = Backend::new();
        backend
            .queue_add(SendCommand {
                id: "slow".into(),
                content: b"S".to_vec(),
                priority: 100,
                interval_ms: 60_000,
            })
            .unwrap();
        backend.queue_start_polling().unwrap();
        thread::sleep(Duration::from_millis(100));

        backend.queue_stop_polling().unwrap();
        // 队列状态立刻变（这是 stop 同步做的）……
        assert!(!backend.queue_status().unwrap().is_polling);
        // ……但线程此刻很可能还活着，所以这一条不能保证成立，
        // 只能保证「在 200ms 分片内必然归零」。
        let deadline = Instant::now() + Duration::from_millis(2000);
        while backend.is_polling_thread_alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !backend.is_polling_thread_alive(),
            "60 秒 interval 的 poller 停止后 2 秒仍未退出"
        );
    }

    /// 停止后重新开始，最终只剩一个线程（无双 poller）。
    #[test]
    fn stop_then_start_leaves_exactly_one_poller_thread() {
        let backend = Backend::new();
        backend
            .queue_add(SendCommand {
                id: "a".into(),
                content: b"A".to_vec(),
                priority: 100,
                interval_ms: 30,
            })
            .unwrap();
        backend.queue_start_polling().unwrap();
        thread::sleep(Duration::from_millis(80));

        backend.queue_stop_polling().unwrap();
        backend.queue_start_polling().unwrap();

        let deadline = Instant::now() + Duration::from_millis(2000);
        while backend.polling_thread_count() != 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            backend.polling_thread_count(),
            1,
            "旧 poller 一直没退出，与新 poller 一起发同一轮"
        );
        backend.queue_stop_polling().unwrap();
    }

    #[test]
    fn timed_tx_bytes_starts_at_zero() {
        let backend = Backend::new();
        assert_eq!(backend.timed_tx_bytes(), 0);
    }
}
