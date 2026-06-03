//! 环形缓冲区性能基准
//!
//! 运行：`cargo bench --bench ring_buffer_bench`
//! 报告：`target/criterion/ring_buffer_bench/`

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use oh_my_serial::RingBuffer;

fn bench_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_write");
    let payload = vec![0xABu8; 4096]; // 4KB
    group.throughput(Throughput::Bytes(payload.len() as u64));

    group.bench_function("write_4KB", |b| {
        let mut buf = RingBuffer::new(65536);
        b.iter(|| {
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
            buf.write(black_box(&payload));
            black_box(buf.drain_all());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_write, bench_write_read_cycle, bench_drain_all);
criterion_main!(benches);
