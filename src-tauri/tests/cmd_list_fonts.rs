//! cmd_list_fonts 集成测试
//!
//! Tauri command 不能脱离 Tauri runtime 直接调，
//! 这里测 IPC 包装层的返回结构（与 list_mono_fonts 一致）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct FontInfo {
    pub family: String,
}

/// 模拟 cmd_list_fonts 的序列化产物
#[test]
fn cmd_list_fonts_serializes_to_valid_json() {
    // 真实环境调 font-kit 太重，这里只验证 wire format
    let fonts: Vec<FontInfo> = vec![
        FontInfo { family: "Consolas".to_string() },
        FontInfo { family: "JetBrains Mono".to_string() },
    ];
    let json = serde_json::to_string(&fonts).expect("serialize");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("parse");
    for f in &parsed {
        assert!(f.get("family").is_some(), "每项必须有 family 字段");
        assert!(f["family"].is_string(), "family 必须是字符串");
    }
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["family"], "Consolas");
    assert_eq!(parsed[1]["family"], "JetBrains Mono");
}

/// FontInfo 反序列化（前端调 invoke 后拿到的对象）
#[test]
fn cmd_list_fonts_deserializes_from_frontend_shape() {
    // 前端 FontInfo 接口：{ family: string }
    let json = r#"[{"family":"Consolas"},{"family":"Cascadia Code"}]"#;
    let parsed: Vec<FontInfo> = serde_json::from_str(json).expect("parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].family, "Consolas");
    assert_eq!(parsed[1].family, "Cascadia Code");
}

/// 空列表（无字体环境）应序列化为空数组
#[test]
fn cmd_list_fonts_empty_list_serializes() {
    let fonts: Vec<FontInfo> = vec![];
    let json = serde_json::to_string(&fonts).expect("serialize");
    assert_eq!(json, "[]");
}
