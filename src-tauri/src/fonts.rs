//! 跨平台系统字体扫描
//!
//! 用 font-kit 列出系统已安装的字体 family（不去严格判断等宽，
//! 因为 font-kit 0.14 没有稳定的 is_monospace API，需 load face 太慢）。
//! 等宽判断交给前端 fallback 栈（xterm 自动用 Consolas 兜底）。
//! Windows 走 DirectWrite，macOS 走 CoreText，Linux 走 fontconfig。

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct FontInfo {
    pub family: String,
}

/// 列出系统已安装的字体 family（去重 + 排序）
pub fn list_mono_fonts() -> Vec<FontInfo> {
    use font_kit::source::SystemSource;

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

#[cfg(test)]
mod tests {
    use super::*;

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
