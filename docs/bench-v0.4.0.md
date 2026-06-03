# 性能基准报告 — v0.4.0

> 日期：2026-06-03
> 工具：[Criterion.rs](https://github.com/bheisler/criterion.rs) 0.5
> 平台：Windows 11 Pro / x86_64 / release profile（`lto = true`, `opt-level = "z"`）
> 全部数据基于 100 个采样点（95% 置信区间）

## 1. 跑法

```bash
# 编译并运行全部基准
cargo bench

# 仅运行单个组
cargo bench --bench ring_buffer_bench -- ring_buffer_write
```

报告输出到 `target/criterion/<group>/<bench>/report/index.html`，可用浏览器查看火焰图/箱线图/历史对比。

## 2. 测量目标

| 组 | 函数 | 测的是什么 |
|----|------|-----------|
| `ring_buffer_write` | `RingBuffer::write(4KB)` | 后台读取线程把一帧数据塞进环形缓冲 |
| `ring_buffer_cycle` | `write(256B) + read(all)` | 一帧 flush 进出 |
| `drain_all` | `write(4KB) + drain_all()` | 后台 reader 触发的批量推送 |
| `send_queue` | `remove + add + next_command` (× 256) | UI 增删预设 + 取下一条 |

## 3. 结果（v0.4.0 新基线）

| 基准 | 延迟中位数 | 95% 区间 | 吞吐量 |
|------|-----------|----------|--------|
| `ring_buffer_write/write_4KB` | **25.8 µs** | [25.3, 26.5] µs | 151 MiB/s |
| `ring_buffer_cycle/write_256B_then_read` | **3.77 µs** | [3.72, 3.83] µs | 130 MiB/s |
| `drain_all/drain_4KB` | **60.5 µs** | [59.2, 62.1] µs | 64.5 MiB/s |
| `send_queue/add_256_to_queue` | **3.27 µs** | [3.13, 3.39] µs | —（混合操作） |

> 单位说明：`write_X_then_read` 类的 throughput 是 read 字节数（2×256B = 512B / iter）。

## 4. 与设计目标对照

| 场景 | 目标 | 实测 | 余量 |
|------|------|------|------|
| 115200 baud 持续接收 | 11.52 KB/s | 64 MiB/s（drain） | **×5500** |
| 921600 baud 突发 flush | 92.16 KB/s | 151 MiB/s（write） | **×1600** |
| 16ms 心跳批量 drain | ≤ 16 ms | 60.5 µs | **×260** |
| 预设面板增删 + 取下一条 | < 1 ms | 3.27 µs（256 条规模） | **×300** |

**结论**：所有关键路径延迟均 < 100 µs，吞吐远超常用串口速率（≤ 921600 baud）。当前实现是 **CPU / 内存** 瓶颈，不是 I/O 瓶颈。

## 5. 注意事项 / 优化空间

1. **`write` 字节级循环**：当前是 `for i in 0..n { buf[(pos+i) % cap] = data[i]; }`，编译器无法 SIMD 化（modulo 不允许）。
   - 优化方向：把环形缓冲换成 `chunked` 视图 + 平台 `copy_nonoverlapping`。
   - 预期：4KB 写从 25 µs 降至 ~5 µs（×5 提升）。

2. **`drain_all` 走 `Vec<u8>` 堆分配**：每次 4KB 都 `vec![…]`，分配 + 拷贝。
   - 优化方向：改用 `BytesMut` 复用缓冲；或让后端 `app.emit` 直接 `into()` 序列化。
   - 预期：把 60 µs 降到 20 µs 以内。

3. **SendQueue 的 `O(n log n)` 排序**：`add()` 调 `sort_by(priority)`，256 条规模下 3 µs 够用；
   10K 条时（极端场景）会变 100 µs+，届时可改 `BinaryHeap`（O(log n) 插入）。

4. **基线对齐**：`drain_4KB` vs `write_4KB` 比例 ≈ 2.3×，差额全在 `Vec<u8>` 分配 + 拷贝上，
   与第 2 条优化方向一致。

## 6. 自动化

`target/criterion/estimates.json` 由 Criterion 持久化。下次跑 `cargo bench` 时自动生成
`change/{new,change}` 目录并报告 `Performance has improved/regressed`（p < 0.05 判定）。

CI 集成（待办）：
- 在 GitHub Actions 上跑 `cargo bench -- --output-format bencher` 抓 JSON
- 写入 benchmark tracking 系统（`bencher.dev` / `codspeed`）做趋势监控
