//! 场景 4: 真实串口 → RingBuffer 端到端
//!
//! 注：RingBuffer 纯逻辑测试在 src/serial/ring_buffer.rs 的内联 mod tests 中
//! 本文件只保留需要真实硬件的端到端验证

mod common;
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn test_real_port_echo() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 写入多组数据，验证回环
    let test_cases: Vec<&[u8]> = vec![
        b"hello",
        b"12345",
        b"\x00\x01\x02\x03\xff",
        b"e2e",
    ];

    for payload in &test_cases {
        port.write_all(payload).expect("写入失败");
        port.flush().expect("flush 失败");

        let mut buf = [0u8; 64];
        let mut received = Vec::new();
        let start = std::time::Instant::now();
        while received.len() < payload.len() && start.elapsed() < Duration::from_secs(2) {
            match port.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => received.extend_from_slice(&buf[..n]),
                Err(_) => std::thread::sleep(Duration::from_millis(2)),
            }
        }
        assert_eq!(received.as_slice(), *payload, "回环数据不一致");
    }
}

#[test]
fn test_continuous_streaming() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 持续写入 50 次，模拟流式数据
    let total_packets = 50;
    for i in 0..total_packets {
        let msg = format!("{:04}", i);
        port.write_all(msg.as_bytes()).expect("写入失败");
        if i % 10 == 9 {
            port.flush().expect("flush 失败");
        }
    }
    port.flush().expect("最终 flush 失败");

    // 读取 50*4 = 200 字节
    let mut buf = [0u8; 64];
    let mut received = Vec::new();
    let start = std::time::Instant::now();
    while received.len() < total_packets * 4 && start.elapsed() < Duration::from_secs(5) {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }

    assert_eq!(received.len(), total_packets * 4, "流式数据丢失");
    // 验证顺序：0000 0001 0002 ... 0049
    for i in 0..total_packets {
        let start_idx = i * 4;
        let expected = format!("{:04}", i);
        let actual = std::str::from_utf8(&received[start_idx..start_idx + 4]).unwrap();
        assert_eq!(actual, expected, "第 {} 个包内容不一致", i);
    }
}
