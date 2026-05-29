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
