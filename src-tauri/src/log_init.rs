//! 日志初始化：env_logger 终端 + fern 滚动文件
//!
//! 日志位置：<exe 同目录>/logs/oh-my-serial-YYYY-MM-DD.log
//! 保留最近 7 天，旧文件自动清理

use chrono::Local;
use fern::Dispatch;
use log::LevelFilter;
use std::fs;
use std::path::PathBuf;

/// 初始化日志系统
///
/// 调用一次，应用启动时
pub fn init() {
    let log_dir = log_dir_path();

    // 创建 logs/ 目录
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("[log] 创建日志目录失败: {:?}", e);
        // 降级到仅 env_logger
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("info"),
        )
        .try_init();
        return;
    }

    // 清理 7 天前的旧日志
    cleanup_old_logs(&log_dir, 7);

    let log_file = log_dir.join(format!(
        "oh-my-serial-{}.log",
        Local::now().format("%Y-%m-%d")
    ));

    let file_dispatch = Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Debug)
        .chain(fern::log_file(&log_file).expect("无法创建日志文件"));

    // 组合：文件 + stderr（开发期可见）
    let combined = Dispatch::new()
        .chain(file_dispatch)
        .chain(
            Dispatch::new()
                .level(LevelFilter::Info)
                .chain(std::io::stderr()),
        )
        .apply();

    if let Err(e) = combined {
        eprintln!("[log] 初始化日志失败: {:?}", e);
        // 最后兜底
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("info"),
        )
        .try_init();
    } else {
        log::info!("📝 日志系统已初始化：{}", log_file.display());
    }
}

/// 推断日志目录（exe 同目录优先，回退到 %APPDATA%）
pub fn log_dir_path() -> PathBuf {
    // 优先：exe 同目录下的 logs/
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join("logs");
        }
    }
    // 回退：%APPDATA%/com.ohmyserial.app/logs/
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("com.ohmyserial.app").join("logs");
    }
    // 最后兜底：当前目录
    PathBuf::from("logs")
}

/// 当前日志文件路径（推断）
pub fn current_log_file() -> PathBuf {
    log_dir_path().join(format!(
        "oh-my-serial-{}.log",
        Local::now().format("%Y-%m-%d")
    ))
}

/// 清理 N 天前的日志文件（pub 出来便于测试）
pub fn cleanup_old_logs(dir: &std::path::Path, keep_days: u64) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed.as_secs() > keep_days * 24 * 3600 {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    /// 辅助：filetime crate 没在主依赖列表里，只在测试用
    mod filetime {
        use std::path::Path;
        pub fn set_file_mtime(p: &Path, t: std::time::SystemTime) -> std::io::Result<()> {
            let f = std::fs::OpenOptions::new().write(true).open(p)?;
            f.set_modified(t)?;
            Ok(())
        }
    }

    /// 在临时目录创建 N 个文件，把其中一个 mtime 调到 8 天前
    /// 每个测试用唯一目录名（避免并发跑 cargo test 时互相覆盖）
    fn setup_mixed_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oh-my-serial-log-test-{}-{}",
            std::process::id(),
            label
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // 三个"新"文件
        for i in 0..3 {
            let p = dir.join(format!("new-{i}.log"));
            File::create(&p).unwrap().write_all(b"x").unwrap();
        }
        // 一个"8 天前"的文件
        let old = dir.join("old.log");
        File::create(&old).unwrap().write_all(b"x").unwrap();
        let past = SystemTime::now() - Duration::from_secs(8 * 24 * 3600);
        filetime::set_file_mtime(&old, past).unwrap();
        dir
    }

    #[test]
    fn cleanup_old_logs_keeps_recent() {
        let dir = setup_mixed_dir("default");
        cleanup_old_logs(&dir, 7);
        // 老的应被删，新的留下
        assert!(!dir.join("old.log").exists());
        assert!(dir.join("new-0.log").exists());
        assert!(dir.join("new-1.log").exists());
        assert!(dir.join("new-2.log").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_old_logs_threshold() {
        // 把阈值设成 30 天 → 8 天前的文件应保留
        let dir = setup_mixed_dir("threshold");
        cleanup_old_logs(&dir, 30);
        assert!(dir.join("old.log").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_dir_path_returns_path() {
        // 只验证不 panic 且返回非空
        let p = log_dir_path();
        assert!(!p.as_os_str().is_empty());
    }

    // ===== read_recent_lines 单测 =====

    /// 在临时目录造一个 log 文件，写入若干行
    /// 每个测试用唯一 label 避免 cargo test 并发冲突
    fn write_log_file(label: &str, lines: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oh-my-serial-logread-{}-{}",
            std::process::id(),
            label
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("test.log");
        let content = lines.join("\n") + "\n";
        std::fs::write(&f, content).unwrap();
        f
    }

    #[test]
    fn parse_line_standard() {
        let raw = "[2026-06-04 14:23:15.123] [INFO] [serial-reader] 设备已断开";
        let l = parse_line(raw.as_bytes()).expect("应能解析标准行");
        assert_eq!(l.timestamp, "14:23:15.123");
        assert_eq!(l.level, "INFO");
        assert_eq!(l.target, "serial-reader");
        assert_eq!(l.message, "设备已断开");
    }

    #[test]
    fn parse_line_full_timestamp_includes_date() {
        // full_timestamp 返 "YYYY-MM-DD HH:MM:SS.mmm"
        let raw = b"[2026-06-04 14:23:15.123] [WARN] [foo] msg";
        let l = parse_line(raw).unwrap();
        assert_eq!(l.full_timestamp, "2026-06-04 14:23:15.123");
    }

    #[test]
    fn parse_line_malformed_returns_none() {
        assert!(parse_line(b"not a log line").is_none());
        assert!(parse_line(b"").is_none());
        assert!(parse_line(b"[ts] no level").is_none());
    }

    #[test]
    fn parse_line_message_with_brackets() {
        // message 内部有方括号（如 "[系统] xxx"）也能正确切
        let raw = b"[2026-06-04 14:23:15.123] [ERROR] [foo] [nested] brackets in msg";
        let l = parse_line(raw).unwrap();
        assert_eq!(l.message, "[nested] brackets in msg");
    }

    #[test]
    fn level_at_least_ordering() {
        assert!(level_at_least("ERROR", "WARN"));
        assert!(level_at_least("WARN", "WARN"));
        assert!(level_at_least("ERROR", "DEBUG"));
        assert!(!level_at_least("INFO", "WARN"));
        assert!(!level_at_least("DEBUG", "INFO"));
        // 大小写不敏感
        assert!(level_at_least("error", "warn"));
    }

    #[test]
    fn read_recent_lines_basic() {
        let f = write_log_file(
            "basic",
            &[
                "[2026-06-04 14:23:15.001] [INFO] [a] first",
                "[2026-06-04 14:23:15.002] [INFO] [a] second",
                "[2026-06-04 14:23:15.003] [INFO] [a] third",
            ],
        );
        // 直接传文件路径辅助函数
        let lines = read_recent_lines_from(&f, 0, 10, None).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].message, "first");
        assert_eq!(lines[2].message, "third");
        let _ = fs::remove_file(&f);
    }

    #[test]
    fn read_recent_lines_limit() {
        let f = write_log_file(
            "limit",
            &[
                "[2026-06-04 14:23:15.001] [INFO] [a] 1",
                "[2026-06-04 14:23:15.002] [INFO] [a] 2",
                "[2026-06-04 14:23:15.003] [INFO] [a] 3",
                "[2026-06-04 14:23:15.004] [INFO] [a] 4",
                "[2026-06-04 14:23:15.005] [INFO] [a] 5",
            ],
        );
        // limit=2 → 最新 2 行（顺序：4 然后 5）
        let lines = read_recent_lines_from(&f, 0, 2, None).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].message, "4");
        assert_eq!(lines[1].message, "5");
        let _ = fs::remove_file(&f);
    }

    #[test]
    fn read_recent_lines_offset() {
        let f = write_log_file(
            "offset",
            &[
                "[2026-06-04 14:23:15.001] [INFO] [a] 1",
                "[2026-06-04 14:23:15.002] [INFO] [a] 2",
                "[2026-06-04 14:23:15.003] [INFO] [a] 3",
                "[2026-06-04 14:23:15.004] [INFO] [a] 4",
                "[2026-06-04 14:23:15.005] [INFO] [a] 5",
            ],
        );
        // offset=1, limit=2 → 倒数第 2 个 2 行 = 2,3
        let lines = read_recent_lines_from(&f, 1, 2, None).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].message, "2");
        assert_eq!(lines[1].message, "3");
        let _ = fs::remove_file(&f);
    }

    #[test]
    fn read_recent_lines_level_filter() {
        let f = write_log_file(
            "filter",
            &[
                "[2026-06-04 14:23:15.001] [INFO] [a] info1",
                "[2026-06-04 14:23:15.002] [WARN] [a] warn1",
                "[2026-06-04 14:23:15.003] [INFO] [a] info2",
                "[2026-06-04 14:23:15.004] [ERROR] [a] err1",
            ],
        );
        let lines = read_recent_lines_from(&f, 0, 10, Some("WARN")).unwrap();
        // 只返 WARN + ERROR（顺序：warn1, err1）
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].level, "WARN");
        assert_eq!(lines[1].level, "ERROR");
        let _ = fs::remove_file(&f);
    }

    #[test]
    fn read_recent_lines_malformed_lines_skipped() {
        let f = write_log_file(
            "malformed",
            &[
                "[2026-06-04 14:23:15.001] [INFO] [a] ok",
                "this is garbage",
                "[2026-06-04 14:23:15.002] [INFO] [a] ok2",
            ],
        );
        let lines = read_recent_lines_from(&f, 0, 10, None).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].message, "ok");
        assert_eq!(lines[1].message, "ok2");
        let _ = fs::remove_file(&f);
    }

    #[test]
    fn read_recent_lines_empty_file() {
        let dir = std::env::temp_dir().join(format!(
            "oh-my-serial-logread-empty-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("test.log");
        std::fs::write(&f, "").unwrap();
        let lines = read_recent_lines_from(&f, 0, 10, None).unwrap();
        assert!(lines.is_empty());
        let _ = fs::remove_file(&f);
    }
}

/// 返回当前日志目录路径（前端 IPC 暴露给 UI）
pub fn log_dir_str() -> String {
    log_dir_path()
        .to_str()
        .unwrap_or("(无法解析路径)")
        .to_string()
}

// ==================== 日志读取（前端 LogPanel 用） ====================

/// 单条解析后的日志（前端 JSON 序列化）
#[derive(serde::Serialize, Clone, Debug)]
pub struct LogLine {
    /// "HH:MM:SS.mmm"
    pub timestamp: String,
    /// "YYYY-MM-DD HH:MM:SS.mmm"（带日期，备用）
    pub full_timestamp: String,
    /// "DEBUG" | "INFO" | "WARN" | "ERROR"
    pub level: String,
    /// log target，如 "serial-reader"
    pub target: String,
    /// 日志正文
    pub message: String,
}

/// 解析一行标准 fern 格式日志
///
/// 格式：`[YYYY-MM-DD HH:MM:SS.mmm] [LEVEL] [target] message`
/// 解析失败返 None（让调用方决定：跳过 / 整行塞进 message）
pub fn parse_line(raw: &[u8]) -> Option<LogLine> {
    let s = std::str::from_utf8(raw).ok()?;
    // 必须以 '[' 开头
    let rest = s.strip_prefix('[')?;
    // 找第一个 ']' 切 timestamp
    let end_ts = rest.find(']')?;
    let ts_part = rest[..end_ts].trim();
    let rest = &rest[end_ts + 1..];
    // 跳空白
    let rest = rest.trim_start();
    // 必须以 '[' 开头（level）
    let rest = rest.strip_prefix('[')?;
    let end_lv = rest.find(']')?;
    let level_part = rest[..end_lv].trim();
    let rest = &rest[end_lv + 1..];
    let rest = rest.trim_start();
    // 必须以 '[' 开头（target）
    let rest = rest.strip_prefix('[')?;
    let end_tg = rest.find(']')?;
    let target_part = rest[..end_tg].trim();
    let message = rest[end_tg + 1..].trim_start().to_string();
    if ts_part.is_empty() || level_part.is_empty() || target_part.is_empty() {
        return None;
    }
    // 时间戳拆日期 + HH:MM:SS.mmm
    let (full_timestamp, timestamp) = match ts_part.split_once(' ') {
        Some((_date, time)) => (ts_part.to_string(), time.to_string()),
        None => (ts_part.to_string(), String::new()),
    };
    Some(LogLine {
        timestamp,
        full_timestamp,
        level: level_part.to_string(),
        target: target_part.to_string(),
        message,
    })
}

/// 级别排序（DEBUG < INFO < WARN < ERROR）
const LEVEL_ORDER: &[&str] = &["DEBUG", "INFO", "WARN", "ERROR"];

/// actual 级别是否 >= min 级别（不区分大小写）
pub fn level_at_least(actual: &str, min: &str) -> bool {
    let a = LEVEL_ORDER
        .iter()
        .position(|&l| l.eq_ignore_ascii_case(actual));
    let m = LEVEL_ORDER
        .iter()
        .position(|&l| l.eq_ignore_ascii_case(min));
    match (a, m) {
        (Some(a), Some(m)) => a >= m,
        _ => false,
    }
}

/// 从指定文件读最近 N 行（暴露给单测）
///
/// `offset=0, limit=N` → 最新 N 行（正序：最早的在前）
/// `offset=K, limit=N` → 倒数第 K+1 个 N 行
/// `level_filter=Some("WARN")` → 只返 WARN 及以上
///
/// 简单实现：一次 read_to_end + split('\n')。v1.0.0 够用；大文件场景
/// v1.1+ 改 4KB 块反向扫（seek_to_end + 按行倒推）。
pub fn read_recent_lines_from(
    path: &std::path::Path,
    offset: u32,
    limit: u32,
    level_filter: Option<&str>,
) -> std::io::Result<Vec<LogLine>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    if limit == 0 {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    // 按行切，过滤空行
    let raw_lines: Vec<&str> = content.split('\n').filter(|l| !l.is_empty()).collect();
    if raw_lines.is_empty() {
        return Ok(Vec::new());
    }
    // 计算切片范围
    let skip = (offset as usize).saturating_mul(limit as usize);
    let end = raw_lines.len().saturating_sub(skip);
    let start = end.saturating_sub(limit as usize);
    // 解析 + 过滤
    let mut out: Vec<LogLine> = raw_lines[start..end]
        .iter()
        .filter_map(|line| parse_line(line.as_bytes()))
        .filter(|l| match level_filter {
            Some(f) => level_at_least(&l.level, f),
            None => true,
        })
        .collect();
    // 正序：slice 已经是正序（早的在前），但 skip 从尾部取，所以早的在前
    // 上面 [start..end] 是正序的子集，直接保留即可
    out.shrink_to_fit();
    Ok(out)
}

/// 读今天日志文件的最近 N 行（IPC 入口）
pub fn read_recent_lines(
    offset: u32,
    limit: u32,
    level_filter: Option<&str>,
) -> std::io::Result<Vec<LogLine>> {
    read_recent_lines_from(&current_log_file(), offset, limit, level_filter)
}

