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
fn log_dir_path() -> PathBuf {
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

/// 清理 N 天前的日志文件
fn cleanup_old_logs(dir: &std::path::Path, keep_days: u64) {
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
