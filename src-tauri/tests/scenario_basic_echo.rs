//! 场景 1: 基础 echo 收发
//!
//! 打开 CH340 → 写入数据 → TX-RX 短接回环 → 读到同样数据

mod common;
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn test_basic_echo_round_trip() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    let payload = b"Hello, OhMySerial!";
    port.write_all(payload).expect("写入失败");
    port.flush().expect("flush 失败");

    let mut buf = [0u8; 64];
    let mut received = Vec::new();
    let start = std::time::Instant::now();
    while received.len() < payload.len() && start.elapsed() < Duration::from_secs(2) {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }

    assert_eq!(received.len(), payload.len(), "数据丢失");
    assert_eq!(received, payload, "数据内容不一致");
}

#[test]
fn test_echo_chinese_payload() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    let payload = "你好,世界! 🚀".as_bytes();
    port.write_all(payload).expect("写入失败");
    port.flush().expect("flush 失败");

    let mut buf = [0u8; 128];
    let mut received = Vec::new();
    let start = std::time::Instant::now();
    while received.len() < payload.len() && start.elapsed() < Duration::from_secs(2) {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }

    assert_eq!(received, payload, "UTF-8 数据回环失败");
}

#[test]
fn test_echo_binary_payload() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 所有 256 个字节值
    let payload: Vec<u8> = (0..=255).collect();
    port.write_all(&payload).expect("写入失败");
    port.flush().expect("flush 失败");

    let mut buf = [0u8; 256];
    let mut received = Vec::new();
    let start = std::time::Instant::now();
    while received.len() < payload.len() && start.elapsed() < Duration::from_secs(3) {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }

    assert_eq!(received, payload, "二进制数据回环失败");
}
