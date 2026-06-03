//! 场景 5: 发送队列轮询
//!
//! 验证多命令按优先级/间隔循环发送
//! CH340 TX-RX 短接，写入立即回环读取
//!
//! 注意：本测试**不在独立线程跑**，而是单线程依次写读各命令，
//! 模拟"轮询线程"的核心语义（不并发、确定性强）。

mod common;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

fn read_until(port: &mut Box<dyn serialport::SerialPort>, target: usize, timeout: Duration) -> Vec<u8> {
    let mut buf = [0u8; 256];
    let mut received = Vec::with_capacity(target);
    let start = Instant::now();
    while received.len() < target && start.elapsed() < timeout {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    received
}

fn write_chunked(port: &mut Box<dyn serialport::SerialPort>, data: &[u8]) {
    const CHUNK: usize = 256;
    for chunk in data.chunks(CHUNK) {
        let mut written = 0;
        let start = Instant::now();
        while written < chunk.len() && start.elapsed() < Duration::from_secs(2) {
            match port.write(&chunk[written..]) {
                Ok(0) => std::thread::sleep(Duration::from_millis(1)),
                Ok(n) => written += n,
                Err(_) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }
    port.flush().ok();
}

/// 模拟轮询：依次发送 N 条命令
/// 真实场景下由 `send-poller` 后台线程循环执行
fn poll_once(
    port: &mut Box<dyn serialport::SerialPort>,
    commands: &[&[u8]],
) {
    for cmd in commands {
        write_chunked(port, cmd);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn test_polling_2_commands() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    let commands: Vec<&[u8]> = vec![b"AT\r\n", b"AT+RST\r\n"];

    // 轮询 3 轮：6 次写入
    let start = Instant::now();
    for _ in 0..3 {
        poll_once(&mut port, &commands);
    }
    let elapsed = start.elapsed();

    // 6 次 * 写入字节 ≈ 21 字节
    let total_sent: usize = commands.iter().map(|c| c.len()).sum::<usize>() * 3;
    let received = read_until(&mut port, total_sent, Duration::from_secs(2));

    assert!(
        received.len() >= total_sent,
        "轮询写入丢失: 期望 {} 字节，实际 {} 字节",
        total_sent,
        received.len()
    );
    eprintln!(
        "✅ 轮询 3 轮完成: 发送 {} 字节, 接收 {} 字节, 耗时 {:?}",
        total_sent,
        received.len(),
        elapsed
    );
}

#[test]
fn test_polling_priority_ordering() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 模拟 SendQueue 排序：高优先级先发
    // (priority=100 的命令排在 priority=10 之前)
    let ordered: Vec<&[u8]> = vec![b"HIGH", b"low"];

    poll_once(&mut port, &ordered);

    // 读取并验证顺序
    let received = read_until(&mut port, 7, Duration::from_secs(2));
    assert!(received.starts_with(b"HIGH"), "高优先级应先发送");
}

#[test]
fn test_polling_stop_resumes() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 启动
    port.write_all(b"start").expect("写失败");
    port.flush().expect("flush 失败");
    let r1 = read_until(&mut port, 5, Duration::from_secs(2));
    assert_eq!(r1, b"start");

    // 短暂空闲（停止模拟）
    std::thread::sleep(Duration::from_millis(100));

    // 重启
    port.write_all(b"resume").expect("写失败");
    port.flush().expect("flush 失败");
    let r2 = read_until(&mut port, 6, Duration::from_secs(2));
    assert_eq!(r2, b"resume");
}
