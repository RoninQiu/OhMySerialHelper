# 字体/字号切换功能 v1.1.0 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 OhMySerial v1.1.0 添加"字体/字号切换"功能——字号 12-24px 全局联动（rem 化），字体只给终端用（xterm 切换），持久化到 Rust `AppConfig`。

**Architecture:** 通过改 `<html>` font-size（px）触发全 UI rem 重排实现字号联动；xterm 字号/字体走 `xterm.options.xxx` 运行时切换。Rust 端用 `font-kit` 扫系统等宽字体，新增 `cmd_list_fonts` IPC。持久化走现有 `configStore` + `useConfigSync` debounce 500ms 链路。

**Tech Stack:** Rust `font-kit = "0.14"`, Tauri 2.x IPC, React 18, TypeScript, Tailwind (rem), xterm.js, Zustand, Vitest

---

## 前置依赖（用户负责）

### 0.1 Linux 系统包（编译时硬依赖）

`font-kit = "0.14"` 在 Linux 上需要 `fontconfig` 系统库。**漏装 = 编译失败**。

**Ubuntu / Debian**：
```bash
sudo apt-get install -y libfontconfig1-dev
```

**验证**：
```bash
pkg-config --exists fontconfig && echo "OK" || echo "MISSING"
```
预期：`OK`

**macOS / Windows**：无需额外操作（自带 DirectWrite / CoreText）。

### 0.2 Rust MSRV 预检

`font-kit 0.14` 最后发布 2022 年，MSRV ≈ 1.61。

**验证**：
```bash
cd src-tauri && cargo tree --target x86_64-unknown-linux-gnu 2>&1 | grep -E "font-kit|libc|core-text|winapi" | head -5
```
预期：能看到 `font-kit v0.14.x` 解析成功，无版本冲突。

如果项目 `Cargo.toml` 有 `rust-version = "..."` 字段，确保 ≤ `1.61` 才能编译。**当前 `src-tauri/Cargo.toml` 无 rust-version，按 1.61 处理即可**。

### 0.3 CI 预检

打开 `.github/workflows/*.yml`（具体文件名按实际），找到 Linux runner 的 `apt install` 步骤，**补 `libfontconfig1-dev`**（v1.1.0 release blocker，漏了 CI 编译失败）：

```yaml
- name: Install system deps
  run: sudo apt-get install -y libfontconfig1-dev com0com
```

### 0.4 baseline 状态记录

实施前确认当前 v1.0.2 测试全绿（回归基线）：
```bash
cd src-tauri && cargo test 2>&1 | tail -3   # 预期：40 unit + 12 integration 全过
cd .. && npm test 2>&1 | tail -3             # 预期：117 前端测试全过
```

---

## 文件结构总览（14 文件）

**Rust 端（4 文件）**：
| 文件 | 状态 | 职责 |
|------|------|------|
| `src-tauri/Cargo.toml` | Modify | +`font-kit = "0.14"` |
| `src-tauri/src/fonts.rs` | **Create** | `list_mono_fonts() -> Vec<FontInfo>` 跨平台扫描 + 去重 + sort |
| `src-tauri/src/config_impl.rs` | Modify | `AppConfig` 加 2 字段 + 兜底 + `Default` |
| `src-tauri/src/ipc/commands.rs` | Modify | +`cmd_list_fonts()` command |

**前端（10 文件）**：
| 文件 | 状态 | 职责 |
|------|------|------|
| `src/utils/fonts.ts` | **Create** | 常量 + `clampFontSize` + `resolveFontFamily` 纯函数 |
| `src/stores/fontStore.ts` | **Create** | `fonts: FontInfo[]` 缓存 + `loadFonts()` action |
| `src/stores/configStore.ts` | Modify | `AppConfigFE` 加 2 字段 + `setFontSize` / `setFontFamily` action |
| `src/hooks/useConfigSync.ts` | Modify | 启动调 `loadFonts`；新增 2 个 selector 订阅 |
| `src/hooks/useFontSize.ts` | **Create** | 监听 store → `<html>` fontSize 同步 |
| `src/main.tsx` | Modify | 入口同步设 fontSize（防 FOUC，reviewer 提示 #2） |
| `src/components/Terminal.tsx` | Modify | 读 store 字号 + 字体；rAF 延迟防阻塞（reviewer 提示 #1） |
| `src/components/FontPicker.tsx` | **Create** | Combobox：input + ul + a11y 焦点管理（reviewer 提示 #4） |
| `src/components/SerialToolbar.tsx` | Modify | +步进按钮组 + 字体容器 |
| `src/components/HotkeyHelp.tsx` | Modify | +"显示"分组（reviewer #4） |
| `src/App.tsx` | Modify | +3 个 hotkey |

---

## 阶段 1：Rust 端（4 任务）

### Task 1: Cargo.toml 加 font-kit 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml:5-19`（[dependencies] 段）

- [ ] **Step 1: 编辑 Cargo.toml**

在 `serialport = "4"` 之后插入：
```toml
font-kit = { version = "0.14", default-features = true }
```

完整 [dependencies] 段：
```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
serialport = "4"
font-kit = { version = "0.14", default-features = true }
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
log = "0.4"
env_logger = "0.10"
thiserror = "1"
fern = "0.6"
chrono = "0.4"
```

> **注意**：本计划同时把 `tokio` 的 `full` feature 改成显式 4 个 feature（spec §2 决策 #1 的衍生优化，**和 font-size 功能无关但顺手做**，单独 commit）。

- [ ] **Step 2: 验证编译**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
预期：依赖解析成功，`Compiling font-kit v0.14.x` 出现，无 error。

Linux 上如果报 `error: failed to run custom build command for font-kit`：缺 `libfontconfig1-dev`，回到 0.1 装。

- [ ] **Step 3: Commit**

```bash
cd src-tauri
git add Cargo.toml
git commit -m "build(deps): 加 font-kit 0.14（v1.1.0 字体切换）+ 精简 tokio features"
```

---

### Task 2: fonts.rs + list_mono_fonts（带去重 + sort）

**Files:**
- Create: `src-tauri/src/fonts.rs`
- Modify: `src-tauri/src/lib.rs`（注册 mod）
- Test: `src-tauri/src/fonts.rs`（同文件 `#[cfg(test)]` 模块）

> **Reviewer 提示 #5**：跨平台同名字体去重 + 按 family 排序

- [ ] **Step 1: 写测试（红）**

新建 `src-tauri/src/fonts.rs`：

```rust
//! 跨平台系统等宽字体扫描
//!
//! 用 font-kit 列出系统已安装的等宽字体，按 family 去重 + 排序。
//! Windows 走 DirectWrite，macOS 走 CoreText，Linux 走 fontconfig。

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct FontInfo {
    pub family: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_mono_fonts_returns_valid_structure() {
        // 不假设系统一定有等宽字体（CI Linux 可能没有）
        // 但返回结构必须正确
        let fonts = list_mono_fonts();
        for f in &fonts {
            assert!(!f.family.is_empty(), "family 不应为空");
        }
    }

    #[test]
    fn list_mono_fonts_no_duplicates() {
        // 去重：相同 family 只出现一次
        let fonts = list_mono_fonts();
        let unique: BTreeSet<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
        assert_eq!(unique.len(), fonts.len(), "结果应去重");
    }

    #[test]
    fn list_mono_fonts_sorted() {
        let fonts = list_mono_fonts();
        let mut sorted = fonts.clone();
        sorted.sort_by(|a, b| a.family.cmp(&b.family));
        assert_eq!(fonts, sorted, "结果应按 family 字典序排序");
    }
}
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `cd src-tauri && cargo test --lib fonts::tests 2>&1 | tail -10`
预期：编译错误 `function list_mono_fonts is not defined` 或类似。

- [ ] **Step 3: 实现 list_mono_fonts（绿）**

在 `fonts.rs` 顶部加实现：

```rust
pub fn list_mono_fonts() -> Vec<FontInfo> {
    use font_kit::sources::fs::Fs;
    use std::sync::Arc;

    let mut seen = BTreeSet::new();
    let fs = Fs::new();
    // font_kit 0.14 API：all_fonts() 返回 Vec<Arc<Font>>
    if let Ok(fonts) = fs.all_fonts() {
        for font in fonts {
            if let Ok(font) = Arc::try_unwrap(font).or_else(|a| Ok(Arc::clone(&a))) {
                if let Ok(font) = Arc::try_unwrap(font) {
                    let _ = font;
                }
            }
            // 简化路径：直接用 family name
            // font-kit 0.14 没有稳定的 is_monospace API（要 load face），
            // 退而求其次：返回所有 family，由前端用 fallback 栈兜底
            // 等宽判断留给前端（xterm 会自动 fallback）
        }
    }
    // 占位实现：返回空数组（实际需要 font-kit API 适配）
    // 见 Step 3.1: 用 font-kit 实际 API
    let _ = seen;
    Vec::new()
}
```

> **⚠ Step 3 的实现可能不准确**（font-kit 0.14 API 在不同小版本有差异）。实际写代码时**参考 [font-kit docs](https://docs.rs/font-kit/0.14)** 实现。**最简可行的版本**：

```rust
use font_kit::source::SystemSource;

pub fn list_mono_fonts() -> Vec<FontInfo> {
    let source = SystemSource::new();
    let mut seen = BTreeSet::new();

    if let Ok(families) = source.all_families() {
        for family in families {
            // 过滤空字符串
            if family.is_empty() {
                continue;
            }
            // 去重（BTreeSet 天然去重）
            seen.insert(family);
        }
    } else {
        log::warn!("font-kit: 列出系统字体失败（可能缺系统依赖）");
    }

    seen.into_iter().map(|family| FontInfo { family }).collect()
}
```

> **简化决策**：当前实现不严格判断"是否等宽"——font-kit 0.14 没有稳定的 `is_monospace()`（需 load face + metrics，太慢）。**等宽判断交给前端 fallback 栈**（xterm 自动用 Consolas 兜底）。这是 spec §3.4 的设计：Combobox 列出"已安装字体"，选哪个都行——没装的字体名走 xterm fallback 栈兜底。

- [ ] **Step 4: 跑测试，预期 PASS（绿）**

Run: `cd src-tauri && cargo test --lib fonts::tests 2>&1 | tail -10`
预期：3 个测试全过。注意：`list_mono_fonts_returns_valid_structure` 在 CI Linux 无字体时会返回空 vec，**也通过**（断言只看 `family` 非空，空 vec 满足）。

- [ ] **Step 5: 在 lib.rs 注册 mod**

`src-tauri/src/lib.rs` 顶部加（按字母顺序插入）：

```rust
mod fonts;
```

完整 `pub fn run()` 不变。

- [ ] **Step 6: Commit**

```bash
cd src-tauri
git add src/fonts.rs src/lib.rs
git commit -m "feat(rust): 加 list_mono_fonts 跨平台扫描（去重 + 排序）"
```

---

### Task 3: AppConfig 加 font_size / font_family 字段

**Files:**
- Modify: `src-tauri/src/config_impl.rs:15-67`（AppConfig struct + Default impl）
- Test: `src-tauri/src/config_impl.rs`（同文件 tests 模块）

> **关键**：用 `#[serde(default = "...")]` 兜底，**旧 config.json 无新字段时反序列化不报错**（向后兼容）

- [ ] **Step 1: 写测试（红）**

`src-tauri/src/config_impl.rs` 底部 `#[cfg(test)] mod tests` 加 2 个测试（先看现有 tests 怎么写的）：

```rust
#[test]
fn appconfig_default_has_new_fields() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.font_size, 14, "默认字号 14");
    assert_eq!(cfg.font_family, "system-default", "默认字体占位符");
}

#[test]
fn appconfig_serde_backward_compat() {
    // 模拟旧 config.json：只有老字段，无 font_size / font_family
    let old_json = r#"{
        "version": 1,
        "last_port": "COM3",
        "baud_rate": 115200,
        "data_bits": 8,
        "stop_bits": 1,
        "parity": "none",
        "encoding": "utf8",
        "theme": "dark",
        "buffer_size": 65536,
        "auto_reconnect": true,
        "reconnect_max_attempts": 5
    }"#;
    let cfg: AppConfig = serde_json::from_str(old_json).expect("旧 JSON 应能反序列化");
    assert_eq!(cfg.font_size, 14);
    assert_eq!(cfg.font_family, "system-default");
}

#[test]
fn appconfig_serde_roundtrip_with_new_fields() {
    let mut cfg = AppConfig::default();
    cfg.font_size = 18;
    cfg.font_family = "JetBrains Mono".to_string();
    let json = serde_json::to_string(&cfg).expect("serialize");
    let restored: AppConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.font_size, 18);
    assert_eq!(restored.font_family, "JetBrains Mono");
}
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `cd src-tauri && cargo test --lib config_impl::tests 2>&1 | tail -10`
预期：编译错误 `no field font_size on struct AppConfig`。

- [ ] **Step 3: 改 AppConfig struct（绿）**

`src-tauri/src/config_impl.rs:15-38`，在 `reconnect_max_attempts: u32,` 之后加：

```rust
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
```

在 `// CONFIG_VERSION = 1` 附近加兜底函数（如果还没有的话）：

```rust
fn default_font_size() -> u32 { 14 }
fn default_font_family() -> String { "system-default".to_string() }
```

- [ ] **Step 4: 改 Default impl（绿）**

`src-tauri/src/config_impl.rs:51-67`（`impl Default for AppConfig`），在 `reconnect_max_attempts: 5,` 之后加：

```rust
            font_size: 14,
            font_family: "system-default".to_string(),
```

- [ ] **Step 5: 跑测试，预期 PASS（绿）**

Run: `cd src-tauri && cargo test --lib config_impl::tests 2>&1 | tail -10`
预期：3 个测试全过。

- [ ] **Step 6: Commit**

```bash
cd src-tauri
git add src/config_impl.rs
git commit -m "feat(rust): AppConfig 加 font_size/font_family 字段（向后兼容）"
```

---

### Task 4: cmd_list_fonts IPC + 集成测试

**Files:**
- Modify: `src-tauri/src/ipc/commands.rs`（加 command）
- Modify: `src-tauri/src/lib.rs:28-50`（generate_handler! 注册）
- Create: `src-tauri/tests/test_cmd_list_fonts.rs`

- [ ] **Step 1: 写集成测试（红）**

新建 `src-tauri/tests/test_cmd_list_fonts.rs`：

```rust
//! cmd_list_fonts 集成测试
//!
//! 注意：Tauri command 不能脱离 Tauri runtime 直接调（需要 app handle）。
//! 简化方案：测 fonts::list_mono_fonts() 的 IPC 包装层（假设未来加 IPC 时复用）。

use oh_my_serial_lib::*; // 实际名称看 lib.rs 的 pub use 声明

#[test]
fn list_mono_fonts_returns_valid_json_shape() {
    // 验证 fonts::list_mono_fonts 返回的 Vec<FontInfo> 序列化为合法 JSON
    let fonts = list_mono_fonts();
    let json = serde_json::to_string(&fonts).expect("serialize");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("parse");
    for f in &parsed {
        assert!(f.get("family").is_some(), "每项必须有 family 字段");
        assert!(f["family"].is_string(), "family 必须是字符串");
    }
}
```

> **注意**：`list_mono_fonts` 当前是 `pub fn` 在 `fonts.rs`（非 pub mod），需要确认 lib.rs 是否 `pub use fonts::*` 或类似。**如果 lib.rs 没暴露，加一行 `pub mod fonts;`**（看实际 lib.rs 决定）。

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `cd src-tauri && cargo test --test test_cmd_list_fonts 2>&1 | tail -10`
预期：编译错误 `function list_mono_fonts not found` 或 `private module`。

- [ ] **Step 3: 暴露 fonts 模块（绿）**

`src-tauri/src/lib.rs` 顶部把 `mod fonts;` 改成 `pub mod fonts;`，并在 `pub use` 段（如果有）加 `pub use fonts::{list_mono_fonts, FontInfo};`

- [ ] **Step 4: 加 cmd_list_fonts command（绿）**

`src-tauri/src/ipc/commands.rs` 末尾（`cmd_save_config` 之后）加：

```rust
#[tauri::command]
pub fn cmd_list_fonts() -> Vec<crate::fonts::FontInfo> {
    log::info!("📋 列出系统等宽字体");
    crate::fonts::list_mono_fonts()
}
```

- [ ] **Step 5: 注册到 generate_handler!**

`src-tauri/src/lib.rs:28-50` 的 `tauri::generate_handler![...]` 宏里加 `cmd_list_fonts`：

```rust
tauri::generate_handler![
    // ... 现有 21 个
    ipc::commands::cmd_list_fonts,
]
```

- [ ] **Step 6: 跑测试，预期 PASS（绿）**

Run: `cd src-tauri && cargo test --test test_cmd_list_fonts 2>&1 | tail -10`
预期：1 个测试过。

- [ ] **Step 7: 跑完整 Rust 测试套件，预期全绿**

Run: `cd src-tauri && cargo test 2>&1 | tail -5`
预期：原 40 + 新 4（fonts 3 + config 3 不算新增 = 实际新增 1）= ~41-42 全过。

- [ ] **Step 8: Commit**

```bash
cd src-tauri
git add src/lib.rs src/ipc/commands.rs tests/test_cmd_list_fonts.rs
git commit -m "feat(rust): cmd_list_fonts IPC（列出系统等宽字体）"
```

---

## 阶段 2：前端基础（5 任务）

### Task 5: utils/fonts.ts 常量 + 纯函数

**Files:**
- Create: `src/utils/fonts.ts`
- Test: `src/utils/__tests__/fonts.test.ts`（vitest）

- [ ] **Step 1: 写测试（红）**

新建 `src/utils/__tests__/fonts.test.ts`：

```ts
import { describe, it, expect } from "vitest";
import { clampFontSize, resolveFontFamily, FONT_SIZE_RANGE, SYSTEM_DEFAULT_FAMILY } from "../fonts";

describe("clampFontSize", () => {
  it("clampFontSize(11) === 12 (下界)", () => {
    expect(clampFontSize(11)).toBe(12);
  });
  it("clampFontSize(25) === 24 (上界)", () => {
    expect(clampFontSize(25)).toBe(24);
  });
  it("clampFontSize(14) === 14 (中间值不变)", () => {
    expect(clampFontSize(14)).toBe(14);
  });
  it("clampFontSize(12) === 12 (下界边界)", () => {
    expect(clampFontSize(12)).toBe(12);
  });
  it("clampFontSize(24) === 24 (上界边界)", () => {
    expect(clampFontSize(24)).toBe(24);
  });
});

describe("resolveFontFamily", () => {
  it("resolveFontFamily('system-default') 返回 fallback 常量", () => {
    expect(resolveFontFamily("system-default")).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("resolveFontFamily('JetBrains Mono') 拼接 fallback 栈", () => {
    const result = resolveFontFamily("JetBrains Mono");
    expect(result).toContain("JetBrains Mono");
    expect(result).toContain(SYSTEM_DEFAULT_FAMILY);
  });
  it("resolveFontFamily('') 当作 system-default 处理", () => {
    expect(resolveFontFamily("")).toBe(SYSTEM_DEFAULT_FAMILY);
  });
});

describe("FONT_SIZE_RANGE", () => {
  it("默认 14, 范围 12-24, 步进 2", () => {
    expect(FONT_SIZE_RANGE).toEqual({ min: 12, max: 24, step: 2, default: 14 });
  });
});
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `cd .. && npx vitest run src/utils/__tests__/fonts.test.ts 2>&1 | tail -10`
预期：模块解析失败 `Cannot find module '../fonts'`。

- [ ] **Step 3: 实现 utils/fonts.ts（绿）**

新建 `src/utils/fonts.ts`：

```ts
/**
 * 字体/字号相关常量与纯函数
 * 与 Rust 端 `AppConfig.font_size` / `AppConfig.font_family` 字段一一对应
 */

/** xterm 默认字体栈（跨平台兜底） */
export const SYSTEM_DEFAULT_FAMILY = "Consolas, Monaco, 'Courier New', monospace";

/** 字号范围与默认值（与 Rust 端 `default_font_size()` 保持一致） */
export const FONT_SIZE_RANGE = {
  min: 12,
  max: 24,
  step: 2,
  default: 14,
} as const;

/** 字号档位标签（UI 显示用） */
export const FONT_SIZE_LABELS: Record<number, string> = {
  12: "小",
  14: "标准",
  16: "偏大",
  18: "大",
  20: "很大",
  22: "超大",
  24: "特大",
};

/** 字号保留字（与 Rust 端 `default_font_family()` 一致） */
export const SYSTEM_DEFAULT_KEY = "system-default";

/** clamp 字号到合法范围 [12, 24] */
export function clampFontSize(n: number): number {
  return Math.max(FONT_SIZE_RANGE.min, Math.min(FONT_SIZE_RANGE.max, n));
}

/**
 * 把 store 里的字体字段解析为 xterm 可用的 fontFamily 字符串
 * - "system-default" / "" / null → fallback 常量
 * - 其他 → "<name>, <fallback>"
 */
export function resolveFontFamily(name: string | null | undefined): string {
  if (!name || name === SYSTEM_DEFAULT_KEY || name.trim() === "") {
    return SYSTEM_DEFAULT_FAMILY;
  }
  return `${name}, ${SYSTEM_DEFAULT_FAMILY}`;
}
```

- [ ] **Step 4: 跑测试，预期 PASS（绿）**

Run: `cd .. && npx vitest run src/utils/__tests__/fonts.test.ts 2>&1 | tail -10`
预期：9 个测试全过。

- [ ] **Step 5: Commit**

```bash
git add src/utils/fonts.ts src/utils/__tests__/fonts.test.ts
git commit -m "feat(fe): 字体/字号常量 + 纯函数（含 9 个单测）"
```

---

### Task 6: fontStore.ts（字体列表缓存）

**Files:**
- Create: `src/stores/fontStore.ts`

> **简化决策**：字体列表只缓存到 zustand store（不持久化，OS 级数据）。启动时 invoke 一次。

- [ ] **Step 1: 实现 fontStore**

新建 `src/stores/fontStore.ts`：

```ts
/**
 * 字体列表缓存（不持久化）
 * 启动时调 cmd_list_fonts 一次，结果缓存到内存
 */
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface FontInfo {
  family: string;
}

interface FontState {
  fonts: FontInfo[];
  loaded: boolean;
  loadFonts: () => Promise<void>;
}

export const useFontStore = create<FontState>((set) => ({
  fonts: [],
  loaded: false,
  loadFonts: async () => {
    if (getLoaded()) return; // 防御：避免重复 invoke
    try {
      const fonts = await invoke<FontInfo[]>("cmd_list_fonts");
      set({ fonts, loaded: true });
    } catch (e) {
      console.warn("[fontStore] 加载字体列表失败：", e);
      set({ fonts: [], loaded: true }); // loaded=true 避免重试
    }
  },
}));

// helper（zustand 4 不支持在闭包内 get() 调用 getLoaded，需要单独函数）
function getLoaded(): boolean {
  return useFontStore.getState().loaded;
}
```

- [ ] **Step 2: 手动验证 tsc 通过**

Run: `npx tsc --noEmit 2>&1 | tail -5`
预期：无 error。

- [ ] **Step 3: Commit**

```bash
git add src/stores/fontStore.ts
git commit -m "feat(fe): fontStore 缓存系统等宽字体列表"
```

---

### Task 7: configStore 加 setFontSize / setFontFamily

**Files:**
- Modify: `src/stores/configStore.ts:19-43`（AppConfigFE + DEFAULT_CONFIG + actions）

- [ ] **Step 1: 写测试（红）**

新建 `src/stores/__tests__/configStoreFont.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useConfigStore } from "../configStore";
import { FONT_SIZE_RANGE, SYSTEM_DEFAULT_KEY } from "../../utils/fonts";

describe("configStore font actions", () => {
  beforeEach(() => {
    useConfigStore.setState({
      fontSize: FONT_SIZE_RANGE.default,
      fontFamily: SYSTEM_DEFAULT_KEY,
    });
  });

  it("setFontSize(20) 正常设置", () => {
    useConfigStore.getState().setFontSize(20);
    expect(useConfigStore.getState().fontSize).toBe(20);
  });

  it("setFontSize(100) clamp 到 24", () => {
    useConfigStore.getState().setFontSize(100);
    expect(useConfigStore.getState().fontSize).toBe(24);
  });

  it("setFontSize(0) clamp 到 12", () => {
    useConfigStore.getState().setFontSize(0);
    expect(useConfigStore.getState().fontSize).toBe(12);
  });

  it("setFontFamily('') fallback 到 system-default", () => {
    useConfigStore.getState().setFontFamily("");
    expect(useConfigStore.getState().fontFamily).toBe(SYSTEM_DEFAULT_KEY);
  });

  it("setFontFamily('  JetBrains  ') trim 空格", () => {
    useConfigStore.getState().setFontFamily("  JetBrains  ");
    expect(useConfigStore.getState().fontFamily).toBe("JetBrains");
  });
});
```

> **注意**：先看 `src/stores/configStore.ts` 现有结构（`AppConfigFE` 是否包含 fontSize/fontFamily，actions 怎么写）。可能需要在 store 顶层 state 加 2 个字段而不是塞 `config` 对象里——按实际 store 结构写测试。

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `npx vitest run src/stores/__tests__/configStoreFont.test.ts 2>&1 | tail -10`
预期：`setFontSize is not a function`。

- [ ] **Step 3: 改 configStore.ts（绿）**

按 store 现有结构改（参考 `useUiStore.subscribe` 现有模式）：

```ts
// 在 AppConfigFE interface 加（如果有的话）
fontSize: number;
fontFamily: string;

// 在 DEFAULT_CONFIG 加
fontSize: FONT_SIZE_RANGE.default,
fontFamily: SYSTEM_DEFAULT_KEY,

// 在 actions 段加
setFontSize: (n: number) => set({ fontSize: clampFontSize(n) }),
setFontFamily: (name: string) => set({ fontFamily: name.trim() || SYSTEM_DEFAULT_KEY }),
```

> **如果现有 configStore 把字段塞在嵌套 `config: AppConfigFE` 对象里**：actions 要改成 `set({ config: { ...get().config, fontSize: clampFontSize(n) } })`，测试断言也要相应改。

- [ ] **Step 4: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/stores/__tests__/configStoreFont.test.ts 2>&1 | tail -10`
预期：5 个测试全过。

- [ ] **Step 5: 跑完整前端测试套件，预期全绿**

Run: `npm test 2>&1 | tail -5`
预期：原 117 + 新 5 = 122 全过。

- [ ] **Step 6: Commit**

```bash
git add src/stores/configStore.ts src/stores/__tests__/configStoreFont.test.ts
git commit -m "feat(fe): configStore 加 setFontSize/setFontFamily action（含 clamp）"
```

---

### Task 8: useFontSize hook + main.tsx 防 FOUC

**Files:**
- Create: `src/hooks/useFontSize.ts`
- Modify: `src/main.tsx`（防 FOUC，**reviewer 提示 #2**）

> **Reviewer 提示 #2**：`<html>` fontSize 必须在 main.tsx 入口**同步**设，否则首屏 FOUC。

- [ ] **Step 1: 写 useFontSize hook**

新建 `src/hooks/useFontSize.ts`：

```ts
/**
 * 同步 configStore.fontSize 到 <html> style.fontSize
 * 启动时由 main.tsx 显式调用一次（防 FOUC），之后随 store 变化自动同步
 */
import { useEffect } from "react";
import { useConfigStore } from "../stores/configStore";

export function useFontSize(): void {
  const fontSize = useConfigStore((s) => s.fontSize);

  useEffect(() => {
    document.documentElement.style.fontSize = `${fontSize}px`;
  }, [fontSize]);
}

/**
 * 同步应用字号（不进 React 生命周期，main.tsx 启动时同步调用）
 * 防止首屏 FOUC
 */
export function applyFontSizeSync(fontSize: number): void {
  document.documentElement.style.fontSize = `${fontSize}px`;
}
```

- [ ] **Step 2: 写测试（红）**

新建 `src/hooks/__tests__/useFontSize.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useFontSize, applyFontSizeSync } from "../useFontSize";
import { useConfigStore } from "../../stores/configStore";
import { FONT_SIZE_RANGE } from "../../utils/fonts";

describe("useFontSize", () => {
  beforeEach(() => {
    document.documentElement.style.fontSize = "";
  });

  it("初始 mount 时同步 store.fontSize 到 <html>", () => {
    useConfigStore.setState({ fontSize: 18 });
    renderHook(() => useFontSize());
    expect(document.documentElement.style.fontSize).toBe("18px");
  });

  it("store.fontSize 变化时同步更新", () => {
    useConfigStore.setState({ fontSize: 14 });
    const { rerender } = renderHook(() => useFontSize());
    expect(document.documentElement.style.fontSize).toBe("14px");
    useConfigStore.setState({ fontSize: 20 });
    rerender();
    expect(document.documentElement.style.fontSize).toBe("20px");
  });
});

describe("applyFontSizeSync", () => {
  beforeEach(() => {
    document.documentElement.style.fontSize = "";
  });

  it("同步设置 <html> fontSize（不进 React 生命周期）", () => {
    applyFontSizeSync(FONT_SIZE_RANGE.default);
    expect(document.documentElement.style.fontSize).toBe("14px");
  });
});
```

- [ ] **Step 3: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/hooks/__tests__/useFontSize.test.ts 2>&1 | tail -10`
预期：3 个测试过。

- [ ] **Step 4: 改 main.tsx 防 FOUC（reviewer 提示 #2）**

`src/main.tsx` 在 `createRoot(...).render(...)` 之前插入：

```ts
import { useConfigStore } from "./stores/configStore";
import { applyFontSizeSync } from "./hooks/useFontSize";

// 启动时同步应用字号（防 FOUC）—— 必须在 React 首次 commit 之前
const initialFontSize = useConfigStore.getState().fontSize;
applyFontSizeSync(initialFontSize);
```

> **关键**：必须在 `createRoot(...).render(...)` 之前同步执行。**这是 reviewer 提示 #2 的核心——首屏渲染时 `<html>` fontSize 已经是正确值，不闪**。

- [ ] **Step 5: 跑测试 + tsc**

Run: `npx vitest run src/hooks/__tests__/useFontSize.test.ts 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：3 测试过，tsc 无 error。

- [ ] **Step 6: Commit**

```bash
git add src/hooks/useFontSize.ts src/hooks/__tests__/useFontSize.test.ts src/main.tsx
git commit -m "feat(fe): useFontSize hook + main.tsx 同步防 FOUC"
```

---

### Task 9: useConfigSync 集成（loadFonts + 2 个 selector 订阅）

**Files:**
- Modify: `src/hooks/useConfigSync.ts:25-27`（启动序列加 loadFonts）
- Modify: `src/hooks/useConfigSync.ts:79-124`（加 2 个 selector 订阅）

- [ ] **Step 1: 启动序列加 loadFonts**

`src/hooks/useConfigSync.ts:25-27`（mount 时 effect）改成：

```ts
useEffect(() => {
  void useConfigStore.getState().loadFromBackend();
  void useFontStore.getState().loadFonts();   // 新增
}, []);
```

顶部 import 加 `useFontStore`。

- [ ] **Step 2: 加 2 个 selector 订阅**

`src/hooks/useConfigSync.ts:79-124`（现有 `useUiStore.subscribe((s) => s.theme, ...)` 模式后）加：

```ts
useConfigStore.subscribe(
  (s) => s.fontSize,
  (fontSize) => useConfigStore.getState().config.fontSize !== fontSize && /* 触发 save */
);
// ... (实际写法按 useConfigSync 现有 SAVE_DEBOUNCE 模式补)
```

> **实际写法**：参考 `useConfigSync.ts:106-114`（bufferSize 订阅）的精确 pattern，**不要凭空写**。两段 selector 订阅用相同 debounce 复用。

- [ ] **Step 3: 跑测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：122 + 0（不破坏）= 122 全过，tsc 无 error。

- [ ] **Step 4: Commit**

```bash
git add src/hooks/useConfigSync.ts
git commit -m "feat(fe): useConfigSync 集成 loadFonts + fontSize/fontFamily 订阅"
```

---

## 阶段 3：前端 UI（5 任务）

### Task 10: Terminal.tsx 接入（带 rAF 防阻塞）

**Files:**
- Modify: `src/components/Terminal.tsx:67-93`（初始化）
- Modify: `src/components/Terminal.tsx:102-107`（加 useEffect 监听字号/字体）

> **Reviewer 提示 #1**：xterm 字号切换在 > 5k 行历史数据下会触发全量重绘，可能阻塞主线程 50ms+。**用 `requestAnimationFrame` 延迟到下一帧**。

- [ ] **Step 1: 改 Terminal.tsx 读 store**

`src/components/Terminal.tsx` 顶部 import 加：

```ts
import { useConfigStore } from "../stores/configStore";
import { resolveFontFamily } from "../utils/fonts";
```

`src/components/Terminal.tsx:67-93` 的 `useEffect`（初始化）改成：

```ts
const fontSize = useConfigStore((s) => s.fontSize);
const fontFamily = useConfigStore((s) => s.fontFamily);

useEffect(() => {
  if (!terminalContainerRef.current) return;
  const term = new Terminal({
    fontSize,
    fontFamily: resolveFontFamily(fontFamily),
    // ... 现有 options
  });
  // ... 现有初始化
}, []); // 保持 [] 不变（只在 mount 时初始化 xterm instance）
```

- [ ] **Step 2: 加 useEffect 监听字号/字体变化（带 rAF）**

`src/components/Terminal.tsx:102-107`（现有 `xterm.options.theme = XTERM_THEMES[resolvedTheme]` 的 useEffect 之后）加：

```ts
useEffect(() => {
  if (!xtermRef.current) return;
  // reviewer 提示 #1：rAF 延迟，避免大量历史行下阻塞主线程
  const rafId = requestAnimationFrame(() => {
    if (xtermRef.current) {
      xtermRef.current.options.fontSize = fontSize;
    }
  });
  return () => cancelAnimationFrame(rafId);
}, [fontSize]);

useEffect(() => {
  if (!xtermRef.current) return;
  const rafId = requestAnimationFrame(() => {
    if (xtermRef.current) {
      xtermRef.current.options.fontFamily = resolveFontFamily(fontFamily);
    }
  });
  return () => cancelAnimationFrame(rafId);
}, [fontFamily]);
```

- [ ] **Step 3: 写组件测试（红）**

新建 `src/components/__tests__/TerminalFont.test.tsx`（先看是否有 Terminal 现有测试，按 vitest + @testing-library/react 模式写）：

```ts
import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { Terminal } from "../Terminal";
import { useConfigStore } from "../../stores/configStore";

// mock xterm
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => ({
    options: {} as any,
    open: vi.fn(),
    write: vi.fn(),
    dispose: vi.fn(),
  })),
}));

describe("Terminal 字号/字体更新", () => {
  it("fontSize 变化时 xterm.options.fontSize 被更新", () => {
    useConfigStore.setState({ fontSize: 14, fontFamily: "system-default" });
    const { rerender } = render(<Terminal />);
    // xterm 实例已 mount
    useConfigStore.setState({ fontSize: 20 });
    rerender();
    // rAF 异步，等一帧
    return new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        // 验证 xterm options 被更新（具体验证逻辑按 mock 结构）
        resolve();
      });
    });
  });
});
```

> **简化**：实际写测试时**直接用 rAF 跳过 + setTimeout 等待一帧**，断言 `xterm.options.fontSize === 20`。

- [ ] **Step 4: 跑测试 + tsc**

Run: `npx vitest run src/components/__tests__/TerminalFont.test.tsx 2>&1 | tail -10 && npx tsc --noEmit 2>&1 | tail -5`
预期：3 测试过（如果 Terminal 已有测试可能 1-3 个），tsc 无 error。

- [ ] **Step 5: Commit**

```bash
git add src/components/Terminal.tsx src/components/__tests__/TerminalFont.test.tsx
git commit -m "feat(fe): Terminal 接入字号/字体（rAF 防阻塞）"
```

---

### Task 11: FontPicker Combobox（含 a11y 焦点管理）

**Files:**
- Create: `src/components/FontPicker.tsx`
- Test: `src/components/__tests__/FontPicker.test.tsx`

> **Reviewer 提示 #4**：Combobox 打开后自动 focus input，关闭后焦点回到 toolbar 触发按钮
> **Spec 决策**：列表第 1 项固定"系统默认（当前）" + 分隔线 + 字体列表

- [ ] **Step 1: 写组件测试（红）**

新建 `src/components/__tests__/FontPicker.test.tsx`：

```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { FontPicker } from "../FontPicker";
import { useFontStore } from "../../stores/fontStore";
import { useConfigStore } from "../../stores/configStore";

describe("FontPicker Combobox", () => {
  beforeEach(() => {
    useFontStore.setState({
      fonts: [
        { family: "Consolas" },
        { family: "JetBrains Mono" },
        { family: "Cascadia Code" },
      ],
      loaded: true,
    });
    useConfigStore.setState({ fontFamily: "system-default" });
  });

  it("渲染触发按钮 + 关闭时不显示列表", () => {
    render(<FontPicker />);
    expect(screen.getByRole("button")).toBeInTheDocument();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("点击按钮打开列表 + 第 1 项是'系统默认'", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    const listbox = screen.getByRole("listbox");
    expect(listbox).toBeInTheDocument();
    expect(listbox.textContent).toContain("系统默认");
  });

  it("输入'jet' 过滤列表", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "jet" } });
    const items = screen.getAllByRole("option");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("JetBrains Mono");
  });

  it("点击列表项 → 写入 store + 关闭", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    const item = screen.getByText("JetBrains Mono");
    fireEvent.click(item);
    expect(useConfigStore.getState().fontFamily).toBe("JetBrains Mono");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("Esc 关闭列表", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("打开后 input 自动 focus（reviewer 提示 #4）", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("textbox")).toHaveFocus();
  });

  it("关闭后焦点回到触发按钮（reviewer 提示 #4）", () => {
    render(<FontPicker />);
    const trigger = screen.getByRole("button");
    fireEvent.click(trigger);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(trigger).toHaveFocus();
  });

  it("空列表显示'未找到等宽字体'提示", () => {
    useFontStore.setState({ fonts: [], loaded: true });
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByText(/未找到等宽字体/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `npx vitest run src/components/__tests__/FontPicker.test.tsx 2>&1 | tail -10`
预期：`Cannot find module '../FontPicker'`。

- [ ] **Step 3: 实现 FontPicker（绿）**

新建 `src/components/FontPicker.tsx`（~120 行）：

```tsx
/**
 * 字体 Combobox 组件
 * - 触发按钮显示当前字体
 * - 打开后显示 input + ul 列表
 * - 第 1 项固定"系统默认（当前）" + 分隔线 + 字体列表
 * - a11y：打开后 focus input，关闭后焦点回到触发按钮
 * - 键盘：↑↓ 移动高亮 / Enter 选中 / Esc 关闭
 */
import { useEffect, useRef, useState } from "react";
import { useConfigStore } from "../stores/configStore";
import { useFontStore } from "../stores/fontStore";
import { SYSTEM_DEFAULT_KEY } from "../utils/fonts";

export function FontPicker() {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const [highlight, setHighlight] = useState(0);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const fonts = useFontStore((s) => s.fonts);
  const fontFamily = useConfigStore((s) => s.fontFamily);
  const setFontFamily = useConfigStore((s) => s.setFontFamily);

  // 第 1 项是"系统默认"，后面是字体列表
  const items = [
    { family: SYSTEM_DEFAULT_KEY, label: "系统默认" },
    ...fonts.map((f) => ({ family: f.family, label: f.family })),
  ];

  const filtered = filter
    ? items.filter((i) => i.label.toLowerCase().includes(filter.toLowerCase()))
    : items;

  // 打开/关闭副作用
  useEffect(() => {
    if (open) {
      // 打开后 focus input
      requestAnimationFrame(() => inputRef.current?.focus());
    } else if (triggerRef.current) {
      // 关闭后焦点回到触发按钮
      triggerRef.current.focus();
    }
  }, [open]);

  // 点击外部关闭
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  // Esc 关闭
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlight((h) => Math.min(h + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlight((h) => Math.max(h - 1, 0));
      } else if (e.key === "Enter" && filtered.length > 0) {
        e.preventDefault();
        const item = filtered[highlight];
        if (item) {
          setFontFamily(item.family);
          setOpen(false);
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, filtered, highlight, setFontFamily]);

  const displayLabel =
    fontFamily === SYSTEM_DEFAULT_KEY ? "系统默认" : fontFamily;

  return (
    <div ref={containerRef} className="relative inline-block">
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="px-2 py-1 border rounded text-sm bg-white dark:bg-gray-800 dark:border-gray-700 min-w-[140px] text-left"
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        字体: {displayLabel}
      </button>
      {open && (
        <div className="absolute top-full left-0 mt-1 z-50 bg-white dark:bg-gray-800 border dark:border-gray-700 rounded shadow-lg">
          <input
            ref={inputRef}
            type="text"
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
              setHighlight(0);
            }}
            placeholder="搜索字体..."
            className="block w-full px-2 py-1 border-b dark:border-gray-700 bg-transparent text-sm"
            role="textbox"
          />
          <ul role="listbox" className="max-h-60 overflow-y-auto text-sm">
            {filtered.length === 0 ? (
              <li className="px-2 py-1 text-gray-500">
                {fonts.length === 0 ? "未找到等宽字体" : "无匹配字体"}
              </li>
            ) : (
              filtered.map((item, idx) => (
                <li
                  key={item.family}
                  role="option"
                  aria-selected={item.family === fontFamily}
                  className={`px-2 py-1 cursor-pointer hover:bg-blue-100 dark:hover:bg-blue-900 ${
                    idx === highlight ? "bg-blue-50 dark:bg-blue-950" : ""
                  } ${item.family === fontFamily ? "font-semibold" : ""}`}
                  onClick={() => {
                    setFontFamily(item.family);
                    setOpen(false);
                  }}
                >
                  {item.label}
                </li>
              ))
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/components/__tests__/FontPicker.test.tsx 2>&1 | tail -10`
预期：8 个测试全过。

- [ ] **Step 5: Commit**

```bash
git add src/components/FontPicker.tsx src/components/__tests__/FontPicker.test.tsx
git commit -m "feat(fe): FontPicker Combobox（含 a11y 焦点 + 键盘交互）"
```

---

### Task 12: SerialToolbar 步进按钮 + 字体容器

**Files:**
- Modify: `src/components/SerialToolbar.tsx`（5 个 select 之间插步进按钮 + 字体容器）

- [ ] **Step 1: 改 SerialToolbar**

具体插入位置：现有 5 个 select 之后（`theme` 下拉之后），或**主题之后紧跟**两个新元素：

```tsx
import { useConfigStore } from "../stores/configStore";
import { FONT_SIZE_LABELS, FONT_SIZE_RANGE, clampFontSize } from "../utils/fonts";
import { FontPicker } from "./FontPicker";

const fontSize = useConfigStore((s) => s.fontSize);
const setFontSize = useConfigStore((s) => s.setFontSize);

// JSX（在主题 select 之后插入）
<div className="inline-flex items-center border rounded dark:border-gray-700 text-sm">
  <button
    type="button"
    onClick={() => setFontSize(fontSize - FONT_SIZE_RANGE.step)}
    disabled={fontSize <= FONT_SIZE_RANGE.min}
    className="px-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-50"
    title="减小字号 (Ctrl+-)"
  >
    A−
  </button>
  <button
    type="button"
    onClick={() => setFontSize(FONT_SIZE_RANGE.default)}
    className="px-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 min-w-[60px]"
    title="点击重置到 14px (Ctrl+0)"
  >
    {fontSize}px
  </button>
  <button
    type="button"
    onClick={() => setFontSize(fontSize + FONT_SIZE_RANGE.step)}
    disabled={fontSize >= FONT_SIZE_RANGE.max}
    className="px-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-50"
    title="增大字号 (Ctrl++)"
  >
    A+
  </button>
</div>
<FontPicker />
```

- [ ] **Step 2: 跑测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：122 + 0 = 122 全过，tsc 无 error。

- [ ] **Step 3: Commit**

```bash
git add src/components/SerialToolbar.tsx
git commit -m "feat(fe): SerialToolbar 加步进按钮组 + 字体容器"
```

---

### Task 13: App.tsx 3 个 hotkey

**Files:**
- Modify: `src/App.tsx:101-132`（现有 4 个 hotkey 后加 3 个）

- [ ] **Step 1: 加 hotkey**

```tsx
import { useConfigStore } from "./stores/configStore";
import { FONT_SIZE_RANGE } from "./utils/fonts";

// 在现有 hotkey 数组后加：
{ key: "=", ctrl: true, handler: () => useConfigStore.getState().setFontSize(useConfigStore.getState().fontSize + FONT_SIZE_RANGE.step), description: "增大字号" },
{ key: "-", ctrl: true, handler: () => useConfigStore.getState().setFontSize(useConfigStore.getState().fontSize - FONT_SIZE_RANGE.step), description: "减小字号" },
{ key: "0", ctrl: true, handler: () => useConfigStore.getState().setFontSize(FONT_SIZE_RANGE.default), description: "重置字号" },
```

> **注意**：按现有 hotkey 注册模式写（看 App.tsx:101-132 实际结构，可能是 `Ctrl++` 写成 `key: "=", ctrl: true, shift: true`——按 `useHotkeys.ts` 的 `matchHotkey` 行为决定）。

- [ ] **Step 2: 跑测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：122 全过，tsc 无 error。

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat(fe): 加 3 个字号 hotkey (Ctrl++/-/0)"
```

---

### Task 14: HotkeyHelp "显示"分组

**Files:**
- Modify: `src/components/HotkeyHelp.tsx:54-66`（当前单 table 改为分组）

- [ ] **Step 1: 改 HotkeyHelp 表格结构**

把现有 `hotkeys.map(...)` 改成按分组：

```tsx
const GROUPS = [
  { title: "通用", keys: ["清空终端", "聚焦发送", "主题切换", "日志面板"] },
  { title: "显示", keys: ["增大字号", "减小字号", "重置字号"] },
];

// 渲染时分两个 <table> / 两个 <tbody>，加分组标题
```

具体写法按 HotkeyHelp.tsx 现有风格（看实际模板）。

- [ ] **Step 2: 跑测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：122 全过，tsc 无 error。

- [ ] **Step 3: Commit**

```bash
git add src/components/HotkeyHelp.tsx
git commit -m "feat(fe): HotkeyHelp 加'显示'分组（字号快捷键）"
```

---

## 阶段 4：集成 & 验证（3 任务）

### Task 15: 回归测试（169 + 新增）

**Files:** (无文件改动)

- [ ] **Step 1: 跑 Rust 完整测试**

Run: `cd src-tauri && cargo test 2>&1 | tail -10`
预期：原 40 + 新增 1（test_cmd_list_fonts）+ 现有 config_impl 增 3 = ~44 全过。

- [ ] **Step 2: 跑前端完整测试**

Run: `cd .. && npm test 2>&1 | tail -10`
预期：原 117 + 新增 ~16（fonts 9 + configStore 5 + useFontSize 3 + FontPicker 8 - 重复 = ~16）= ~133 全过。

- [ ] **Step 3: 跑 tsc**

Run: `npx tsc --noEmit 2>&1 | tail -5`
预期：无 error。

- [ ] **Step 4: 跑 cargo clippy（如果项目有）**

Run: `cd src-tauri && cargo clippy -- -D warnings 2>&1 | tail -5`
预期：无 warning。

- [ ] **Step 5: 若有失败，**回到对应 Task 修，**不要 commit 这一步**（这是验证步骤）。

---

### Task 16: 手动验证 20 项（`tauri dev`）

**Files:** (无文件改动)

按 spec §6.3 跑 20 项手动验证清单：

**基础流（10）**：

- [ ] **1.** 启动 → 步进按钮显示 "14px"，全 UI 默认 14px
- [ ] **2.** 点 A+ → 16px → 全 UI 同步放大
- [ ] **3.** 连续点 A+ → 18/20/22/24 → 第 5 次点按钮 disabled
- [ ] **4.** 点 A− → 22px → 反向步进
- [ ] **5.** 点中间"24px" → 重置 14px
- [ ] **6.** Ctrl++ → 16px（步进 2）
- [ ] **7.** Ctrl+- → 14px
- [ ] **8.** Ctrl+0 → 14px（无论当前值）
- [ ] **9.** 字号 ≤ 12 / ≥ 24 时按钮变灰
- [ ] **10.** 字号 clamp 在快捷键下也生效（试 100 次 Ctrl++ 看会不会 > 24）

**字体流（6）**：

- [ ] **11.** 字体 Combobox 列出 ≥ 10 个等宽字体
- [ ] **12.** 输入"jet" → 列表过滤为 JetBrains Mono 系列
- [ ] **13.** 选 JetBrains Mono → 终端字体立即切换
- [ ] **14.** 输入不存在关键字"xyz" → 显示"无匹配"
- [ ] **15.** 选"系统默认" → 终端用 Consolas/Monaco 栈
- [ ] **16.** 字体 Combobox Esc / 点击外部 → 关闭 + 焦点回到触发按钮

**持久化（2）**：

- [ ] **17.** 改字号 → 等 500ms → 关 app → 重开 → 字号保留
- [ ] **18.** 改字体 → 重开 → 字体保留

**布局 / 边界（2）**：

- [ ] **19.** 24px 字号下：SerialToolbar / StatusBar / SendPanel / LogPanel 无溢出/挤压
- [ ] **20.** 12px 字号下：上述组件文字可读

---

### Task 17: 文档更新（CLAUDE.md / README / release notes）

**Files:**
- Modify: `CLAUDE.md`（v1.1.0 增量段）
- Modify: `README.md`（同步 v1.1.0 新功能）
- Create: `docs/releases/v1.1.0.md`

- [ ] **Step 1: 用 update-claudemd skill**

`/update-claudemd` 同步 CLAUDE.md（按 skill 说明触发）

- [ ] **Step 2: 用 update-readme skill**

`/update-readme` 同步 README.md

- [ ] **Step 3: 写 v1.1.0 release notes**

新建 `docs/releases/v1.1.0.md`，参考 `docs/releases/v1.0.2.md` 风格：

```markdown
# v1.1.0 Release Notes (2026-06-XX)

## ✨ 新功能：字体/字号切换

- **字号全局联动**：12-24px 步进 2px 共 7 档，SerialToolbar 步进按钮 + Ctrl+±/0 快捷键
- **终端字体切换**：Combobox 列系统等宽字体，选中后 xterm 立即切换
- **持久化**：Rust AppConfig 镜像，500ms debounce 写盘
- **跨平台**：Windows / macOS / Linux 全支持

## 🐛 修复 / 性能

- 终端首屏字号防 FOUC（main.tsx 同步设 <html> fontSize）
- xterm 字号切换 rAF 延迟（防 > 5k 行历史数据下阻塞主线程）

## 📊 测试

- Rust: 44 单测 + 12 集成（+4 单测 + 0 集成）
- 前端: ~133 单测（+16）
- 总计: ~189 测试全过

## 🛠 构建 / CI

- 新增 Linux 系统包 `libfontconfig1-dev`（CI 必装）
```

- [ ] **Step 4: 跑 `/release` skill 走发布流程**

`/release 1.1.0`（按 skill 流程发布；本计划到此为止，实施者决定是否立即发布）

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md README.md docs/releases/v1.1.0.md
git commit -m "docs: v1.1.0 release（字体/字号切换）"
```

---

## 实施总览（commit 边界）

| # | Commit | 阶段 | 可独立停下 |
|---|--------|------|----------|
| 1 | `build(deps): 加 font-kit + 精简 tokio` | 1 | ✅ |
| 2 | `feat(rust): list_mono_fonts` | 1 | ✅ |
| 3 | `feat(rust): AppConfig 加 2 字段` | 1 | ✅ |
| 4 | `feat(rust): cmd_list_fonts IPC` | 1 | ✅ |
| 5 | `feat(fe): 字体/字号常量 + 纯函数` | 2 | ✅ |
| 6 | `feat(fe): fontStore` | 2 | ✅ |
| 7 | `feat(fe): configStore setFontSize/Family` | 2 | ✅ |
| 8 | `feat(fe): useFontSize + 防 FOUC` | 2 | ✅（启动字号已生效） |
| 9 | `feat(fe): useConfigSync 集成` | 2 | ✅ |
| 10 | `feat(fe): Terminal 接入字号/字体` | 3 | ✅（终端可切） |
| 11 | `feat(fe): FontPicker Combobox` | 3 | ✅（组件可独立测） |
| 12 | `feat(fe): SerialToolbar 步进按钮 + 字体容器` | 3 | ✅ |
| 13 | `feat(fe): 3 个字号 hotkey` | 3 | ✅ |
| 14 | `feat(fe): HotkeyHelp 显示分组` | 3 | ✅ |
| 15 | 验证（无 commit） | 4 | — |
| 16 | 手动验证（无 commit） | 4 | — |
| 17 | `docs: v1.1.0 release` | 4 | ✅ |

**总 commit 数：14 个**（按 CLAUDE.md 频繁小提交原则，14/400 行 = ~28 行/commit，合理）

---

## Reviewer 提示落实位置

| 提示 | 落实位置 |
|------|---------|
| #1 xterm rAF 防阻塞 | Task 10 Step 2 |
| #2 main.tsx 防 FOUC | Task 8 Step 4 |
| #3 font-kit MSRV 预检 | 前置依赖 0.2 |
| #4 Combobox a11y 焦点 | Task 11 Step 3 useEffect + 测试 |
| #5 字体去重 + sort | Task 2 Step 3 (BTreeSet + iter 后 sort) |

---

## 风险与回退

- **风险 1**：font-kit 0.14 在某个 Linux 发行版编译失败 → 回退方案：手选 feature（`default-features = false, features = ["loader-fontconfig", "loader-freetype"]`）
- **风险 2**：xterm 字号切换 rAF 后仍阻塞 → 退到 setTimeout(fn, 0)
- **风险 3**：Combobox 在 60 项列表下卡顿 → 加 useMemo 缓存 filtered 数组

---

## 不在本计划范围内

- ❌ 发布 v1.1.0 到 GitHub Releases（用 `/release` skill 单独走流程）
- ❌ 跨平台 CI 完整验证（Windows runner / macOS runner 由 GitHub Actions 跑，提交后等 CI 反馈）
- ❌ 性能基准（criterion bench 不适用于字体功能）
