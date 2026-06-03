use thiserror::Error;

#[derive(Error, Debug)]
pub enum SerialError {
    #[error("串口打开失败: {0}")]
    OpenFailed(String),

    #[error("串口已被占用")]
    PortLocked,

    #[error("发送超时")]
    SendTimeout,

    #[error("接收错误: {0}")]
    ReceiveError(String),

    #[error("缓冲区溢出，已丢弃 {0} 字节")]
    BufferOverflow(usize),

    #[error("串口未打开")]
    PortNotOpen,

    #[error("配置无效: {0}")]
    InvalidConfig(String),
}

impl serde::Serialize for SerialError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

/// 把 std::io::Error 按错误类型分类映射到 SerialError
///
/// 关键映射：
/// - `NotConnected` / `BrokenPipe` → `PortNotOpen`（设备拔出）
/// - `TimedOut` → `ReceiveError("超时")`（不视为断线）
/// - 其他 → `ReceiveError(...)`（累计 N 次后才视为断线）
impl From<std::io::Error> for SerialError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::NotConnected | ErrorKind::BrokenPipe => SerialError::PortNotOpen,
            ErrorKind::TimedOut => SerialError::ReceiveError("读取超时".to_string()),
            _ => SerialError::ReceiveError(err.to_string()),
        }
    }
}
