//! 性能基准（v0.4.0）
//!
//! 运行：`cargo bench`
//! 报告：`target/criterion/`
//!
//! 覆盖关键热路径：
//! - RingBuffer 写 / 读循环 / drain_all
//! - SendQueue 添加与优先级排序
//! - HEX 工具（CRC16 计算）

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oh_my_serial::{RingBuffer, SendCommand, SendQueue};

fn bench_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_write");
    let payload = vec![0xABu8; 4096]; // 4KB
    group.throughput(Throughput::Bytes(payload.len() as u64));

    // 每次迭代前重置缓冲位置（避免后续迭代变 no-op，扭曲结果）
    group.bench_function("write_4KB", |b| {
        let mut buf = RingBuffer::new(65536);
        b.iter(|| {
            // 重置到空（保留底层容量，复用分配）
            buf.reset_for_bench();
            buf.write(black_box(&payload));
        });
    });

    group.finish();
}

fn bench_write_read_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_cycle");
    let payload = vec![0xCDu8; 256]; // 256B (典型 flush 块大小)
    group.throughput(Throughput::Bytes((payload.len() * 2) as u64));

    group.bench_function("write_256B_then_read", |b| {
        let mut buf = RingBuffer::new(65536);
        b.iter(|| {
            buf.reset_for_bench();
            buf.write(black_box(&payload));
            let n = buf.data_len();
            buf.read(black_box(n));
        });
    });

    group.finish();
}

fn bench_drain_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("drain_all");
    let payload: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    group.throughput(Throughput::Bytes(payload.len() as u64));

    group.bench_function("drain_4KB", |b| {
        let mut buf = RingBuffer::new(65536);
        b.iter(|| {
            buf.reset_for_bench();
            buf.write(black_box(&payload));
            black_box(buf.drain_all());
        });
    });

    group.finish();
}

// ==================== SendQueue ====================

/// LCG 状态机：放在 &mut Cell 里避免借用冲突
struct Lcg {
    state: std::cell::Cell<u32>,
}
impl Lcg {
    fn new(seed: u32) -> Self {
        Self {
            state: std::cell::Cell::new(seed),
        }
    }
    fn next_u32(&self) -> u32 {
        let s = self.state.get().wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.state.set(s);
        s
    }
    fn next_id(&self) -> String {
        format!("cmd-{:x}", self.next_u32())
    }
    fn next_priority(&self) -> u8 {
        let s = self.state.get().wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.state.set(s);
        (s >> 16) as u8
    }
}

/// 真实场景：周期性添加 + 偶尔取出，模拟 UI 端用户增删预设
fn bench_send_queue_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("send_queue");

    // 静态命令池（避免迭代中分配污染）
    let pool: Vec<SendCommand> = (0..256)
        .map(|i| SendCommand {
            id: format!("seed-{i}"),
            content: vec![0xAA; 64],
            priority: (i % 200) as u8,
            interval_ms: 100,
        })
        .collect();

    group.bench_function("add_256_to_queue", |b| {
        let mut q = SendQueue::new();
        // 预热：填入 256 条
        for cmd in &pool {
            q.add(cmd.clone());
        }
        let idx = std::cell::Cell::new(0usize);
        let lcg = Lcg::new(0xDEAD_BEEF);
        b.iter(|| {
            // 模拟：删除首条 → 添加新条 → 取下一条
            let i = idx.get();
            let next = &pool[i % pool.len()];
            idx.set(i.wrapping_add(1));
            q.remove(black_box(&next.id));
            let mut fresh = next.clone();
            fresh.id = lcg.next_id();
            fresh.priority = lcg.next_priority();
            q.add(fresh);
            black_box(q.next_command());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_write,
    bench_write_read_cycle,
    bench_drain_all,
    bench_send_queue_cycle
);
criterion_main!(benches);
