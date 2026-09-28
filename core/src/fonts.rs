//! 跨平台系统字体扫描
//!
//! dev 模式默认关闭 font-kit（省 harfbuzz/fontconfig 编译 ~80s），
//! 返回空列表；release 自动启用 `real-fonts` feature，列出真实系统字体。
//! 等宽判断交给前端 fallback 栈（xterm 自动用 Consolas 兜底）。
//! Windows 走 DirectWrite，macOS 走 CoreText，Linux 走 fontconfig。

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct FontInfo {
    pub family: String,
}

/// 列出系统已安装的字体 family。
///
/// - `real-fonts` feature 启用：调用 font-kit 列出真实字体
/// - feature 关闭（dev stub）：返回空 Vec，省 harfbuzz/fontconfig 编译
pub fn list_mono_fonts() -> Vec<FontInfo> {
    #[cfg(feature = "real-fonts")]
    {
        list_with_font_kit()
    }

    #[cfg(not(feature = "real-fonts"))]
    {
        log::info!(
            "font-kit 未启用（dev stub），返回空列表；release 自动启用 real-fonts feature"
        );
        Vec::new()
    }
}

#[cfg(feature = "real-fonts")]
fn list_with_font_kit() -> Vec<FontInfo> {
    use font_kit::source::SystemSource;
    use std::collections::BTreeSet;

    let source = SystemSource::new();
    let mut seen = BTreeSet::new();

    match source.all_families() {
        Ok(families) => {
            for family in families {
                if !family.is_empty() {
                    seen.insert(family);
                }
            }
        }
        Err(e) => {
            log::warn!("font-kit: 列出系统字体失败（{}），返回空列表", e);
        }
    }

    seen.into_iter().map(|family| FontInfo { family }).collect()
}

/// 在常见 CJK 字体家族中查找第一个可用的，返回其 TTF/OTF 字节；找不到返回 None。
///
/// 供宿主 UI（如 egui）注册中文 fallback 字体使用；仅在 `real-fonts` feature 下可用。
#[cfg(feature = "real-fonts")]
pub fn find_cjk_font_bytes() -> Option<Vec<u8>> {
    use font_kit::family_name::FamilyName;
    use font_kit::handle::Handle;
    use font_kit::properties::Properties;
    use font_kit::source::SystemSource;

    // 按出现概率排序的候选（CJK 范围覆盖尽量广）
    const CANDIDATES: &[&str] = &[
        "Microsoft YaHei UI", "Microsoft YaHei",
        "NSimSun", "SimSun",
        "Yu Gothic UI", "MS Gothic", "Malgun Gothic",
        "PingFang SC", "Hiragino Sans GB",
        "Noto Sans CJK SC", "Noto Sans CJK TC", "Noto Sans CJK JP",
    ];

    let source = SystemSource::new();
    let props = Properties::default();
    for name in CANDIDATES {
        let req = FamilyName::Title((*name).to_string());
        if let Ok(handle) = source.select_best_match(&[req], &props) {
            // font-kit 的 Handle 枚举有两种变体：Path（磁盘路径）或 Memory（Arc<Vec<u8>>）
            let bytes = match &handle {
                Handle::Memory { bytes, .. } => Some(bytes.as_slice().to_vec()),
                Handle::Path { path, .. } => std::fs::read(path).ok(),
            };
            if let Some(b) = bytes {
                return Some(b);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn list_mono_fonts_returns_valid_structure() {
        // 不假设系统一定有字体（CI Linux 可能没有），
        // 但返回结构必须正确：每项 family 非空
        let fonts = list_mono_fonts();
        for f in &fonts {
            assert!(!f.family.is_empty(), "family 不应为空");
        }
    }

    #[test]
    fn list_mono_fonts_handles_empty_gracefully() {
        // 即使系统无字体也不应 panic
        let fonts = list_mono_fonts();
        // 允许空 vec 或正常 vec，两种都合法
        for f in &fonts {
            assert!(!f.family.is_empty());
        }
    }

    #[test]
    fn list_mono_fonts_no_duplicates() {
        let fonts = list_mono_fonts();
        let unique: BTreeSet<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
        assert_eq!(unique.len(), fonts.len(), "结果应去重");
    }

    #[test]
    fn list_mono_fonts_sorted() {
        let fonts = list_mono_fonts();
        let mut sorted = fonts.clone();
        sorted.sort_by(|a, b| a.family.cmp(&b.family));
        assert_eq!(fonts, sorted, "结果应按 family 字典序排序（BTreeSet 隐式排序）");
    }
}
