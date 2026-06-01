//! 环境检测测试
//!
//! 验证 CH340 硬件测试环境是否就绪。

mod common;

#[test]
fn test_hardware_port_configured() {
    if !common::hardware_available() {
        return; // 跳过（CI 友好）
    }
    let _guard = common::serial_guard();
    let port = common::test_port().unwrap();
    eprintln!("✅ 使用测试端口: {}", port);
    assert!(!port.is_empty());
}

#[test]
fn test_open_test_port_succeeds() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port = common::test_port().unwrap();
    let result = common::open_test_port(&port);
    assert!(result.is_ok(), "打开串口失败: {:?}", result.err());
}
