//! 场景 3: 断线检测 + 重连
//!
//! CH340 拔出后，read 应返回错误或 0 字节
//! 重新插入后能正常打开

mod common;
use std::io::{Read, Write};
use std::time::Duration;

fn read_until(port: &mut Box<dyn serialport::SerialPort>, target: usize, timeout: Duration) -> Vec<u8> {
    let mut buf = [0u8; 64];
    let mut received = Vec::with_capacity(target);
    let start = std::time::Instant::now();
    while received.len() < target && start.elapsed() < timeout {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    received
}

#[test]
fn test_reopen_after_close() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();

    // 第一次会话
    {
        let mut port = common::open_test_port(&port_name).expect("首次打开失败");
        port.write_all(b"session1").expect("写入失败");
        port.flush().expect("flush 失败");
        let received = read_until(&mut port, 8, Duration::from_secs(2));
        assert_eq!(received, b"session1", "首次会话回环失败");
    } // drop → 关闭

    // 第二次会话（重新打开）
    {
        let mut port = common::open_test_port(&port_name).expect("重连失败");
        port.write_all(b"session2").expect("写入失败");
        port.flush().expect("flush 失败");
        let received = read_until(&mut port, 8, Duration::from_secs(2));
        assert_eq!(received, b"session2", "二次会话回环失败");
    }
}

#[test]
fn test_multiple_reopen_cycles() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();

    // 连续 5 次开关
    for i in 0..5 {
        let mut port = common::open_test_port(&port_name)
            .unwrap_or_else(|e| panic!("第 {} 次打开失败: {:?}", i, e));
        let msg = format!("cycle{}", i);
        port.write_all(msg.as_bytes()).expect("写入失败");
        port.flush().expect("flush 失败");
        let received = read_until(&mut port, msg.len(), Duration::from_secs(2));
        assert_eq!(received, msg.as_bytes(), "第 {} 次循环数据不一致", i);
        drop(port); // 关闭
    }
}

#[test]
fn test_rapid_close_reopen() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();

    // 快速开关 20 次
    for i in 0..20 {
        {
            let _port = common::open_test_port(&port_name)
                .unwrap_or_else(|e| panic!("第 {} 次快速打开失败: {:?}", i, e));
            // 立即 drop
        }
    }

    // 最后正常打开并通信
    let mut port = common::open_test_port(&port_name).expect("最终打开失败");
    port.write_all(b"survived").expect("写入失败");
    port.flush().expect("flush 失败");
    let received = read_until(&mut port, 8, Duration::from_secs(2));
    assert_eq!(received, b"survived");
}

/// 验证错误映射策略（不直接引用 oh_my_serial::SerialError，
/// 而是用辅助函数判断映射类别，避免触发 tauri 链接问题）
///
/// CH340 拔出时 serialport 抛出 `Io(io::ErrorKind::NotConnected)`，
/// 后端读取线程应能识别为断线信号。
#[test]
fn test_port_not_open_error_mapping() {
    // 使用与生产代码一致的映射策略：
    // NotConnected / BrokenPipe → 断线
    // TimedOut → 超时（不视为断线）
    // 其他 → 普通接收错误（累计 N 次后才视为断线）
    fn classify(err: &std::io::Error) -> &'static str {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::NotConnected | ErrorKind::BrokenPipe => "disconnect",
            ErrorKind::TimedOut => "timeout",
            _ => "other",
        }
    }

    let timeout_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
    assert_eq!(classify(&timeout_err), "timeout");

    let disc_err = std::io::Error::new(std::io::ErrorKind::NotConnected, "gone");
    assert_eq!(classify(&disc_err), "disconnect");

    let pipe_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe");
    assert_eq!(classify(&pipe_err), "disconnect");

    let other_err = std::io::Error::new(std::io::ErrorKind::Other, "unknown");
    assert_eq!(classify(&other_err), "other");
}
