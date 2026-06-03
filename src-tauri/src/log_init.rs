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
}

/// 返回当前日志目录路径（前端 IPC 暴露给 UI）
pub fn log_dir_str() -> String {
    log_dir_path()
        .to_str()
        .unwrap_or("(无法解析路径)")
        .to_string()
}

