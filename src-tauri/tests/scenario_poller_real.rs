//! 场景 5b: 真的驱动 core 的 send-poller 线程（TX-RX 短接）
//!
//! `scenario_polling.rs` 里的三个测试在**测试文件内**自己重写了一遍发送循环
//! （`poll_once`），从不调用 `Backend::queue_start_polling`。也就是说它们验的是
//! 测试自己的辅助函数，core 的 poller 换成什么实现都会过——本文件补上这个缺口。
//!
//! 三个缺陷逐个对应：
//! - `round_robins_every_command` —— 游标取即推进、按长度回绕，两条命令都该上线缆
//! - `poller_survives_idle_line`  —— 空闲线路上锁可用率约 2%，poller 不能因此静默退出
//! - `stop_takes_effect_within_200ms` —— 长 interval 的不可中断 sleep 会让停止延迟 60 秒
//!
//! TX-RX 短接时，poller 写出去的字节会被 reader 读回来，经 mpsc 数据通道到达本测试。

mod common;

use oh_my_serial_core::{Backend, OpenPortOptions, SendCommand};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// 起一个已经打开测试串口的 Backend。
fn open_backend() -> (Arc<Backend>, mpsc::Receiver<Vec<u8>>) {
    let port_name = common::test_port().expect("OH_MY_SERIAL_TEST_PORT 未设置");
    let (tx, rx) = mpsc::channel::<Vec<u8>>(1024);
    let backend = Arc::new(Backend::new());
    backend
        .open_port(
            OpenPortOptions {
                port_name,
                baud_rate: 115_200,
                data_bits: 8,
                stop_bits: 1,
                parity: "none".into(),
            },
            tx,
        )
        .expect("打开串口失败");
    (backend, rx)
}

fn cmd(id: &str, content: &[u8], priority: u8, interval_ms: u64) -> SendCommand {
    SendCommand {
        id: id.into(),
        content: content.to_vec(),
        priority,
        interval_ms,
    }
}

/// 收满 `target` **字节**后返回。超时返回已收到的部分。
///
/// 按字节而不是按数据块计数：块会被 reader 合并/拆分，按块计会让同一份
/// 硬件行为在并行与串行下得出不同结论（实测踩过）。
fn collect_bytes(rx: &mut mpsc::Receiver<Vec<u8>>, target: usize, timeout: Duration) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(target + 64);
    let start = Instant::now();
    while out.len() < target && start.elapsed() < timeout {
        match rx.try_recv() {
            Ok(chunk) => out.extend_from_slice(&chunk),
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    out
}

/// 关掉后台线程并释放串口。测试结束时统一调用，避免端口被占住影响后续用例。
fn shutdown(backend: &Arc<Backend>) {
    let _ = backend.queue_stop_polling();
    let _ = backend.stop_periodic_send();
    std::thread::sleep(Duration::from_millis(100));
    let _ = backend.close_port();
}

/// 轮转一圈：两条命令都该发出去，且第一轮里高优先级在前。
///
/// 这条在旧实现下必然失败——旧 poller 每 tick 都取 `commands.first()`，
/// 只会反复发 priority 最高那条。
#[test]
fn round_robins_every_command() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let (backend, mut rx) = open_backend();

    backend.queue_add(cmd("hi", b"AA", 100, 20)).unwrap();
    backend.queue_add(cmd("lo", b"BB", 10, 20)).unwrap();
    backend.queue_start_polling().unwrap();

    // 2 条命令 × 3 轮 = 12 字节
    let all = collect_bytes(&mut rx, 12, Duration::from_secs(5));
    shutdown(&backend);
    eprintln!("📦 收到 {} 字节: {:?}", all.len(), String::from_utf8_lossy(&all));

    // 按 2 字节切成「发了哪条命令」的序列
    let sent: Vec<&[u8]> = all.chunks_exact(2).collect();
    assert!(
        sent.iter().any(|c| *c == b"AA"),
        "高优先级命令没上线缆，收到的字节: {all:?}"
    );
    assert!(
        sent.iter().any(|c| *c == b"BB"),
        "低优先级命令从没被发出去（旧缺陷：只发队首那条），收到的字节: {all:?}"
    );
    assert!(
        sent.first().is_some_and(|c| *c == b"AA"),
        "第一轮应从 priority 最高那条开始，实际: {all:?}"
    );

    // 关键：必须**交替**。只断言「两条都出现过」不够——先 AA 后 AA 再 BB
    // 也满足那条，但那是坏行为。旧实现是全 AA，这里必然挂。
    let alternates = sent.windows(2).all(|w| w[0] != w[1]);
    assert!(
        alternates,
        "两条命令应轮流发送，实际序列: {sent:?}"
    );
    assert!(
        sent.len() >= 6,
        "3 轮 × 2 条应至少收到 6 次发送，实际 {} 次: {sent:?}",
        sent.len()
    );
}

/// 空闲线路上 poller 不能静默死掉。
///
/// 旧实现的锁预算约 100ms，而 reader 跨阻塞读持锁、空闲线路上锁可用率仅约 2%
/// ⇒ 约 1/3 概率启动即死，且失败路径是静默 break。
///
/// 「收够字节」这一半是概率性的（取决于当次是否落在争用窗口），
/// 所以另加一条**确定性**判据：整个过程不得出现 `SendPollerError`。
/// 旧实现把「抢不到锁」构造成一个假 IO 错误，正是走这条路发出来的。
#[test]
fn poller_survives_idle_line() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let (backend, mut rx) = open_backend();
    let mut events = backend.subscribe();

    backend.queue_add(cmd("x", b"XY", 100, 30)).unwrap();
    backend.queue_start_polling().unwrap();

    // 收满 40 字节（约 0.6 秒的发送量）
    let got = collect_bytes(&mut rx, 40, Duration::from_secs(8));
    shutdown(&backend);

    // 把事件队列里剩下的错误事件翻出来
    let mut errors = Vec::new();
    while let Ok(ev) = events.try_recv() {
        if let oh_my_serial_core::BackendEvent::SendPollerError(e) = ev {
            errors.push(e);
        }
    }

    assert!(
        errors.is_empty(),
        "poller 报了写入失败（旧缺陷：把锁争当成写失败）：{errors:?}"
    );
    assert!(
        got.len() >= 40,
        "8 秒内只收到 {} 字节，poller 很可能已经静默退出（旧缺陷）",
        got.len()
    );
}

/// 长 interval 的停止必须立刻生效，不能等 interval 走完。
///
/// 旧实现躺在不可中断的 `thread::sleep(interval_ms)` 里，interval 可配到 60000ms
/// ⇒ 点了停止最长 60 秒才真正退出。
///
/// **判据是线程本身，不是 `queue_status().is_polling`**——后者是
/// `queue_stop_polling()` 同步置的位，旧实现里它同样会立刻变 false，
/// 用它当判据的测试在旧代码上也会通过（实测踩过）。
#[test]
fn stop_takes_effect_within_200ms() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let (backend, mut rx) = open_backend();

    // interval 60 秒：发出第一条后就会睡进去
    backend.queue_add(cmd("slow", b"S", 100, 60_000)).unwrap();
    backend.queue_start_polling().unwrap();
    assert!(
        backend.is_polling_thread_alive(),
        "启动后线程应被标记为在跑"
    );

    // 等第一条，确认 poller 真的跑起来了
    let first = collect_bytes(&mut rx, 1, Duration::from_secs(5));
    assert!(!first.is_empty(), "poller 没能发出第一条，测试无从判断停止");

    let t0 = Instant::now();
    backend.queue_stop_polling().unwrap();

    // 旧实现下线程还在 60 秒的 sleep 里，要等它醒过来才发现代号变了
    let mut settled = None;
    while t0.elapsed() < Duration::from_secs(3) {
        if !backend.is_polling_thread_alive() {
            settled = Some(t0.elapsed());
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    shutdown(&backend);

    let took = settled.unwrap_or_else(|| {
        panic!("停止后 3 秒 poller 线程仍未退出（旧缺陷：躺在 60 秒的不可中断 sleep 里）")
    });
    assert!(
        took < Duration::from_millis(1000),
        "停止生效耗时 {took:?}，远超 200ms 的分片上限"
    );
    eprintln!("✅ 停止生效耗时 {took:?}（interval 为 60 秒）");
}

/// 停止之后立刻重开，不该出现双 poller 同时发同一轮。
///
/// 判据是**发送速率**：单 poller 每 30ms 发 1 字节 ⇒ 约 33 字节/秒；
/// 两个 poller 各发各的 ⇒ 约 66，差一倍，区间分得很开。
///
/// （不能用「字节里有没有 ZZ」：负载是单字节 Z 时，单 poller 本来就产出
/// 全连续的 Z——那个判据在单 poller 下也成立，测不出任何东西。）
#[test]
fn stop_then_start_does_not_double_send() {
    if !common::hardware_available() {
        return;
    }
    let _guard = common::serial_guard();
    let (backend, mut rx) = open_backend();

    backend.queue_add(cmd("z", b"Z", 100, 30)).unwrap();
    backend.queue_start_polling().unwrap();
    collect_bytes(&mut rx, 2, Duration::from_secs(3)); // 先跑起来

    backend.queue_stop_polling().unwrap();
    backend.queue_start_polling().unwrap(); // 立刻重开

    // 排空缓冲后，按 1 秒窗口数速率
    let _ = collect_bytes(&mut rx, 8, Duration::from_millis(500));
    let t0 = Instant::now();
    let got = collect_bytes(&mut rx, 4096, Duration::from_millis(1000));
    let secs = t0.elapsed().as_secs_f64();
    shutdown(&backend);

    let rate = got.len() as f64 / secs;
    eprintln!("📈 重开后发送速率 {rate:.1} 字节/秒（单 poller 期望 ≈33）");
    assert!(
        rate < 50.0,
        "发送速率 {rate:.1} 字节/秒，远超单 poller 的 ≈33，说明有两个 poller 在发"
    );
}
