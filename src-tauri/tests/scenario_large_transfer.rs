//! 场景 2: 大数据量传输
//!
//! 验证 RingBuffer 4KB 阈值触发 flush + 8KB 一次性传输 + 分块传输

mod common;
use std::io::{Read, Write};
use std::time::Duration;

/// 读满 N 字节或超时
fn read_until(port: &mut Box<dyn serialport::SerialPort>, target: usize, timeout: Duration) -> Vec<u8> {
    let mut buf = [0u8; 1024];
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

/// 分批写入（绕过 CH340 单次写入缓冲限制）
/// 每次最多 256 字节，写入失败时短暂等待后重试
fn write_chunked(port: &mut Box<dyn serialport::SerialPort>, data: &[u8]) -> std::io::Result<()> {
    const CHUNK: usize = 256;
    for chunk in data.chunks(CHUNK) {
        let mut written = 0;
        let start = std::time::Instant::now();
        while written < chunk.len() && start.elapsed() < Duration::from_secs(2) {
            match port.write(&chunk[written..]) {
                Ok(0) => std::thread::sleep(Duration::from_millis(1)),
                Ok(n) => written += n,
                Err(_) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        if written < chunk.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "分批写入超时",
            ));
        }
    }
    Ok(())
}

#[test]
fn test_8kb_round_trip() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 8KB 数据 (> 4KB 阈值)
    let payload: Vec<u8> = (0..8192).map(|i| (i % 256) as u8).collect();

    let start = std::time::Instant::now();
    write_chunked(&mut port, &payload).expect("写入失败");
    port.flush().expect("flush 失败");
    let received = read_until(&mut port, 8192, Duration::from_secs(10));
    let elapsed = start.elapsed();

    assert_eq!(received.len(), 8192, "8KB 数据丢失");
    assert_eq!(received, payload, "8KB 数据内容不一致");
    eprintln!("✅ 8KB 传输完成, 耗时: {:?}", elapsed);
}

#[test]
fn test_chunked_8x1kb() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 8 个 1KB 数据块
    let chunks: Vec<Vec<u8>> = (0..8)
        .map(|i| (i * 1024..(i + 1) * 1024).map(|j| (j % 256) as u8).collect())
        .collect();

    for chunk in &chunks {
        write_chunked(&mut port, chunk).expect("写入失败");
        port.flush().expect("flush 失败");
        std::thread::sleep(Duration::from_millis(10));
    }

    let received = read_until(&mut port, 8192, Duration::from_secs(10));
    let expected: Vec<u8> = chunks.iter().flatten().cloned().collect();

    assert_eq!(received.len(), 8192, "分块数据丢失");
    assert_eq!(received, expected, "分块数据内容不一致");
}

#[test]
fn test_burst_small_writes() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).expect("打开串口失败");

    // 100 个 32 字节的小数据（模拟高频小包）
    let total = 100;
    let chunk_size = 32;
    let mut all_sent = Vec::with_capacity(total * chunk_size);
    for i in 0..total {
        let chunk: Vec<u8> = (0..chunk_size).map(|j| ((i * 7 + j) % 256) as u8).collect();
        port.write_all(&chunk).expect("写入失败");
        all_sent.extend(chunk);
    }
    port.flush().expect("flush 失败");

    let received = read_until(&mut port, total * chunk_size, Duration::from_secs(10));
    assert_eq!(received.len(), total * chunk_size, "突发小包数据丢失");
    assert_eq!(received, all_sent, "突发小包数据内容不一致");
}
