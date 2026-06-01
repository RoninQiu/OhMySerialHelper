use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{interval, MissedTickBehavior};

/// 高精度定时发送器
///
/// 设计预留：尚未被 IPC 调用（Task 14 集成）。
/// 通过 `#[allow(dead_code)]` 抑制 unused 警告。
#[allow(dead_code)]
pub struct PreciseSender {
    is_running: bool,
}

#[allow(dead_code)]
impl PreciseSender {
    pub fn new() -> Self {
        Self { is_running: false }
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// 启动定时发送任务
    pub async fn start_sending(
        &mut self,
        port_handle: Arc<Mutex<Option<Box<dyn SerialPort + Send>>>>,
        payload: Vec<u8>,
        interval_ms: u64,
    ) where
        dyn SerialPort + Send: 'static,
    {
        self.is_running = true;

        let mut interval_timer = interval(Duration::from_millis(interval_ms));
        interval_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);

        while self.is_running {
            interval_timer.tick().await;

            let mut handle = port_handle.lock().await;
            if let Some(ref mut port) = *handle {
                if let Err(e) = port.write_all(&payload) {
                    eprintln!("发送失败: {:?}", e);
                    break;
                }
                if let Err(e) = port.flush() {
                    eprintln!("刷新失败: {:?}", e);
                    break;
                }
            } else {
                // 串口未打开，停止发送
                break;
            }
        }

        self.is_running = false;
    }

    /// 停止定时发送
    pub fn stop(&mut self) {
        self.is_running = false;
    }
}

#[allow(dead_code)]
impl Default for PreciseSender {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export SerialPort trait for use in the sender
pub use serialport::SerialPort;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precise_sender_creation() {
        let sender = PreciseSender::new();
        assert!(!sender.is_running());
    }
}
