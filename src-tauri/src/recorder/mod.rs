//! 录制器：把终端显示数据按行写入本地文件
//!
//! v1.2.0 设计：
//! - 纯字符串 sink：调用方负责格式化（去 ANSI），本模块不知道 HEX/TEXT
//! - BufWriter 8KB 攒批写盘（与 reader 节奏一致）
//! - 跨 reader 线程生命周期（Arc<Mutex<Option<Recorder>>>）
//!   重连不切文件，断线/重连事件通过 mark_event 写注释行
//!
//! 文件格式示例：
//! ```
//! # OhMySerial Capture — COM3 @ 115200 8 1 none — 2026-06-22 14:35:12 (v1.2.0)
//! [14:35:12.456] ← [HEX] AA BB CC DD
//! [14:35:12.520] → [HEX] 01 02 03
//! # [14:35:15.789] 设备已断开: BrokenPipe
//! # [14:35:47.123] 重连成功 (gap 31.334s)
//! [14:35:47.500] ← hello world
//! ```

use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

/// BufWriter 缓冲大小（8KB，与 reader 节奏一致）
const BUF_CAPACITY: usize = 8 * 1024;

/// 录制器主体：持有 BufWriter + 累计字节数
///
/// 设计：stop() / write_header() 等方法消费 self；write_line / mark_event / bytes_written
/// 借用 self。这样 BufWriter 在 drop 时自动 flush（即使忘记调 stop）。
#[derive(Debug)]
pub struct Recorder {
    writer: BufWriter<File>,
    bytes_written: u64,
    path: PathBuf,
    started_at: Instant,
}

/// 录制摘要（停止时返回给前端显示）
#[derive(Debug, Clone, Serialize)]
pub struct RecorderSummary {
    pub path: PathBuf,
    pub bytes_written: u64,
    pub duration_ms: u64,
}

/// 创建录制器：打开 path 写入文件（truncate 模式）
pub fn start_recording(path: PathBuf) -> std::io::Result<Recorder> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
    Ok(Recorder {
        writer: BufWriter::with_capacity(BUF_CAPACITY, file),
        bytes_written: 0,
        path,
        started_at: Instant::now(),
    })
}

impl Recorder {
    /// 写入一行纯文本（不含 \n，自动追加 LF）
    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.bytes_written += line.len() as u64 + 1;
        Ok(())
    }

    /// 写一行注释（自动加 "# " 前缀，便于 grep 过滤事件）
    pub fn mark_event(&mut self, text: &str) -> std::io::Result<()> {
        let prefixed = format!("# {}", text);
        self.write_line(&prefixed)
    }

    /// 写文件头元数据（端口/波特率/数据位/停止位/校验位/时间/版本）
    /// 自动加 "# " 前缀，作为注释行
    pub fn write_header(
        &mut self,
        port: &str,
        baud: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: &str,
    ) -> std::io::Result<()> {
        let now = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let line = format!(
            "# OhMySerial Capture — {} @ {} {} {} {} — {} (v1.2.0)",
            port, baud, data_bits, stop_bits, parity, now
        );
        self.write_line(&line)
    }

    /// 已写入字节数（含 \n 字符）
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// 当前文件路径
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// flush + 关闭 + 返回摘要
    pub fn stop(mut self) -> std::io::Result<RecorderSummary> {
        self.writer.flush()?;
        Ok(RecorderSummary {
            path: self.path.clone(),
            bytes_written: self.bytes_written,
            duration_ms: self.started_at.elapsed().as_millis() as u64,
        })
    }
}

/// Drop 实现：忘记调 stop() 时也能 flush 落盘
/// （bufwriter_flushes_on_drop 测试覆盖）
impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::path::PathBuf;

    /// 测试用临时文件路径（按 pid + label 区分，避免 cargo test 并发冲突）
    fn tmp_path(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oh-my-serial-rec-test-{}-{}.txt",
            std::process::id(),
            label
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn start_recording_creates_empty_file() {
        let p = tmp_path("start");
        let rec = start_recording(p.clone()).expect("start_recording 应成功");
        assert!(p.exists(), "文件应被创建");
        assert_eq!(rec.bytes_written(), 0, "初始字节数为 0");
        rec.stop().expect("stop 应成功");
    }

    #[test]
    fn write_line_accumulates_bytes() {
        let p = tmp_path("write");
        let mut rec = start_recording(p.clone()).expect("start 失败");
        rec.write_line("[14:35:12.456] ← AA BB CC")
            .expect("write_line 失败");
        rec.write_line("[14:35:12.520] → 01 02")
            .expect("write_line 失败");
        assert!(
            rec.bytes_written() > 30,
            "至少两行字节数 (>30)，实际: {}",
            rec.bytes_written()
        );
        rec.stop().expect("stop 失败");
        let mut content = String::new();
        std::fs::File::open(&p)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("← AA BB CC"), "应包含 RX 行");
        assert!(content.contains("→ 01 02"), "应包含 TX 行");
    }

    #[test]
    fn mark_event_adds_hash_prefix() {
        let p = tmp_path("event");
        let mut rec = start_recording(p.clone()).expect("start 失败");
        rec.mark_event("设备已断开: BrokenPipe")
            .expect("mark_event 失败");
        rec.stop().expect("stop 失败");
        let content = std::fs::read_to_string(&p).unwrap();
        assert!(content.starts_with("# "), "注释行应以 '# ' 开头");
        assert!(
            content.contains("设备已断开"),
            "注释行应包含原始文本"
        );
    }

    #[test]
    fn stop_returns_correct_summary() {
        let p = tmp_path("stop");
        let mut rec = start_recording(p.clone()).expect("start 失败");
        rec.write_line("hello").expect("write 失败");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let summary = rec.stop().expect("stop 失败");
        assert_eq!(summary.path, p, "summary.path 应等于输入路径");
        assert!(summary.bytes_written > 0, "bytes_written > 0");
        assert!(
            summary.duration_ms >= 10,
            "duration_ms >= 10ms，实际: {}",
            summary.duration_ms
        );
    }

    #[test]
    fn large_write_thousands_of_lines() {
        let p = tmp_path("large");
        let mut rec = start_recording(p.clone()).expect("start 失败");
        for i in 0..10000 {
            rec.write_line(&format!("line {}", i))
                .expect("write 失败");
        }
        let summary = rec.stop().expect("stop 失败");
        assert!(
            summary.bytes_written > 50_000,
            "10000 行应 > 50KB，实际: {}",
            summary.bytes_written
        );
        let content = std::fs::read_to_string(&p).unwrap();
        assert_eq!(content.lines().count(), 10000, "行数应为 10000");
    }

    /// 集成场景 1：同路径重新打开（truncate 模式覆盖旧内容）
    /// 注：原本放在 tests/recorder_integration.rs，但 `use oh_my_serial::*`
    /// 会拉入 Tauri runtime 触发 Windows STATUS_ENTRYPOINT_NOT_FOUND
    /// （详见 config_json_shape.rs 的注释）。改放单元测试。
    #[test]
    fn resume_recording_on_same_path_truncates() {
        let p = tmp_path("resume");
        // 第一个 session
        {
            let mut rec = start_recording(p.clone()).expect("session1 start 失败");
            rec.write_line("session 1 line 1").expect("write 失败");
            rec.stop().expect("session1 stop 失败");
        }
        // 第二个 session - 在同一路径打开（truncate 模式）
        let mut rec2 = start_recording(p.clone()).expect("session2 start 失败");
        rec2.write_line("session 2 line 1")
            .expect("session2 write 失败");
        rec2.stop().expect("session2 stop 失败");

        let mut content = String::new();
        std::fs::File::open(&p)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(
            !content.contains("session 1"),
            "truncate 后不应有 session 1 内容，实际: {content:?}"
        );
        assert!(content.contains("session 2"), "应有 session 2 内容");
    }

    /// 集成场景 2：BufWriter drop 时自动 flush（不调 stop 也能落盘）
    #[test]
    fn bufwriter_flushes_on_drop() {
        let p = tmp_path("drop");
        {
            let mut rec = start_recording(p.clone()).expect("start 失败");
            rec.write_line("drop test").expect("write 失败");
            // 不调 stop()，让 rec drop（触发 BufWriter Drop → flush）
        }
        let content = std::fs::read_to_string(&p).expect("读取失败");
        assert!(
            content.contains("drop test"),
            "drop 后内容应已落盘，实际: {content:?}"
        );
    }

    /// 集成场景 3：多线程并发写入不丢行
    #[test]
    fn concurrent_writes_no_data_loss() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let p = tmp_path("concurrent");
        let rec = Arc::new(Mutex::new(
            start_recording(p.clone()).expect("start 失败"),
        ));

        let mut handles = vec![];
        for t in 0..4 {
            let rec = Arc::clone(&rec);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let mut g = rec.lock().expect("lock 失败");
                    g.write_line(&format!("thread {t} line {i}"))
                        .expect("write 失败");
                }
            }));
        }
        for h in handles {
            h.join().expect("join 失败");
        }
        // 所有线程结束后，Arc 引用计数为 1，可取所有权
        let rec = Arc::try_unwrap(rec).expect("Arc 仍有引用");
        let rec = rec.into_inner().expect("Mutex poisoned");
        rec.stop().expect("stop 失败");

        let content = std::fs::read_to_string(&p).expect("read 失败");
        // 4 线程 × 100 行 = 400 行
        assert_eq!(
            content.lines().count(),
            400,
            "4 线程 × 100 行 = 400 行，实际: {}",
            content.lines().count()
        );
        // 每个线程的每一行都应出现
        for t in 0..4 {
            for i in 0..100 {
                let needle = format!("thread {t} line {i}");
                assert!(content.contains(&needle), "缺少: {needle}");
            }
        }
    }
}