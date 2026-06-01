//! 集成测试公共工具
//!
//! CH340 硬件测试：用户通过 `OH_MY_SERIAL_TEST_PORT` 环境变量指定 COM 号
//! TX-RX 短接实现自发自收（echo 模式）
//! 未设置环境变量时跳过所有硬件测试

use std::env;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// 全局硬件互斥锁 —— CH340 单端口同时只能被一个测试持有
/// 用 OnceLock + Mutex 跨测试文件共享
pub fn hardware_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 从环境变量读取测试用 COM 端口
///
/// 设置方式（PowerShell）：
/// ```powershell
/// $env:OH_MY_SERIAL_TEST_PORT = "COM5"
/// ```
pub fn test_port() -> Option<String> {
    env::var("OH_MY_SERIAL_TEST_PORT")
        .ok()
        .filter(|s| !s.is_empty())
}

/// 检测测试硬件是否就绪
///
/// 检查项：
/// 1. 环境变量已设置
/// 2. 系统能找到该 COM 端口（CH340 驱动已装、已插入）
pub fn hardware_available() -> bool {
    let port = match test_port() {
        Some(p) => p,
        None => {
            eprintln!(
                "⏭ 跳过：未设置 OH_MY_SERIAL_TEST_PORT 环境变量\n\
                 设置方式：$env:OH_MY_SERIAL_TEST_PORT = \"COM5\"\n\
                 (你的 CH340 分配到的 COM 号)"
            );
            return false;
        }
    };

    match serialport::available_ports() {
        Ok(ports) => {
            let exists = ports.iter().any(|p| p.port_name == port);
            if !exists {
                eprintln!(
                    "⏭ 跳过：COM 端口 {} 不存在\n\
                     请检查：\n  1. CH340 是否已插入 USB\n  2. 设备管理器中分配的 COM 号是否正确\n  3. CH340 驱动是否已安装",
                    port
                );
            }
            exists
        }
        Err(e) => {
            eprintln!("⏭ 跳过：枚举串口失败: {}", e);
            false
        }
    }
}

/// 打开指定测试串口（与生产代码配置一致）
pub fn open_test_port(name: &str) -> serialport::Result<Box<dyn serialport::SerialPort>> {
    serialport::new(name, 115200)
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(50))
        .open()
}

/// 测试入口守卫：获取全局硬件锁，确保同一时刻只有一个测试访问 CH340
///
/// 用法：
/// ```ignore
/// #[test]
/// fn test_xxx() {
///     let _guard = common::serial_guard();
///     // ... 串口操作
/// }
/// ```
pub fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    hardware_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
