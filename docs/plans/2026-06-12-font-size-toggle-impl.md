# 字体/字号切换功能 v1.1.0 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 OhMySerial v1.1.0 添加"字体/字号切换"功能——字号 12-24px 全局联动（rem 化），字体只给终端用（xterm 切换），持久化到 Rust `AppConfig`。

**Architecture:** 通过改 `<html>` font-size（px）触发全 UI rem 重排实现字号联动；xterm 字号/字体走 `xterm.options.xxx` 运行时切换。Rust 端用 `font-kit` 扫系统等宽字体，新增 `cmd_list_fonts` IPC。前端复用现有 `configStore` 嵌套结构（`config: AppConfigFE`）+ `useConfigSync` sync/save 模式，不新建独立 store。

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

### 0.3 硬编码 px 尺寸审计（spec §3.6 + reviewer MAJOR #3）

**目的**：12-24px 字号下确保布局不崩。**这是 v1.1.0 pre-merge checklist**。

**步骤**：

1. 跑 grep：
   ```bash
   cd .. && grep -rnE "(\bw-\[|h-\[|min-h-\[|p-\[|m-\[|gap-\[)" src/ | head -30
   ```

2. 人工分类每条命中：
   - **A. 溢出风险**（按钮/select 宽度固定，字号变大文字溢出）→ 改 `w-[NNrem]` 或加 `overflow-x-auto`
   - **B. 高度不足**（按钮固定 32px 高，字号 24px 装不下）→ 改 `min-h-[2.5rem]` 或加 `h-auto`
   - **C. 间距紧**（padding 8px 字号 24px 显得挤）→ 改 `p-[0.5rem]`
   - **D. 忽略**（图标 / 边框 / 浮层宽度等不随字号变化）

3. 把分类结果写到本计划末尾的"实施期检查清单"段。

4. **如果分类结果是 A/B/C**（需要改 ≥ 1 处），新增 **Task 1.5: 硬编码 px 修复** 在 Rust 端 4 task 之后跑（见后）。

### 0.4 baseline 状态记录

实施前确认当前 v1.0.2 测试全绿：
```bash
cd src-tauri && cargo test 2>&1 | tail -3   # 预期：40 unit + 12 integration 全过
cd .. && npm test 2>&1 | tail -3             # 预期：117 前端测试全过
```

---

## 文件结构总览（15 文件）

**Rust 端（4 文件）**：
| 文件 | 状态 | 职责 |
|------|------|------|
| `src-tauri/Cargo.toml` | Modify | +`font-kit = "0.14"` + 精简 tokio features |
| `src-tauri/src/fonts.rs` | **Create** | `list_mono_fonts() -> Vec<FontInfo>` 跨平台扫描 + 去重 + sort |
| `src-tauri/src/config_impl.rs` | Modify | `AppConfig` 加 `font_size: u32` + `font_family: String` + 兜底 + `Default` |
| `src-tauri/src/ipc/commands.rs` | Modify | +`cmd_list_fonts()` command |

**前端（11 文件）**：
| 文件 | 状态 | 职责 |
|------|------|------|
| `src/utils/fonts.ts` | **Create** | 常量 + `clampFontSize` + `resolveFontFamily` 纯函数 |
| `src/stores/fontStore.ts` | **Create** | `fonts: FontInfo[]` 缓存 + `loadFonts()` action |
| `src/stores/configStore.ts` | Modify | `AppConfigFE` 加 2 字段 + `setFontSize` / `setFontFamily` action |
| `src/hooks/useConfigSync.ts` | Modify | sync() 保留 2 字段 + 新增 2 个 selector 订阅 |
| `src/hooks/useFontSize.ts` | **Create** | 监听 store → `<html>` fontSize 同步 |
| `src/main.tsx` | Modify | 入口同步设 fontSize（防 FOUC，reviewer 提示 #2） |
| `src/components/Terminal.tsx` | Modify | 读 store 字号 + 字体；rAF 延迟防阻塞（reviewer 提示 #1） |
| `src/components/FontPicker.tsx` | **Create** | Combobox：input + ul + a11y 焦点管理 + 键盘 ↑↓（reviewer 提示 #4） |
| `src/components/SerialToolbar.tsx` | Modify | +步进按钮组 + 字体容器 |
| `src/components/HotkeyHelp.tsx` | Modify | +"显示"分组 |
| `src/App.tsx` | Modify | +3 个 hotkey |

**CI（1 文件）**：
| 文件 | 状态 | 职责 |
|------|------|------|
| `.github/workflows/test.yml`（或对应文件名） | Modify | 加 `libfontconfig1-dev` |

---

## 阶段 1：Rust 端（5 任务）

### Task 1: Cargo.toml 加 font-kit + 精简 tokio

**Files:**
- Modify: `src-tauri/Cargo.toml:5-19`（[dependencies] 段）

- [ ] **Step 1: 编辑 Cargo.toml**

`tokio` 行改为：
```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

`serialport = "4"` 之后插入：
```toml
font-kit = { version = "0.14", default-features = true }
```

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

### Task 1.5: CI workflow 加 libfontconfig1-dev（reviewer MINOR #8）

**Files:**
- Modify: `.github/workflows/*.yml`（找到 Linux runner 的 `apt install` 步骤）

> **Reviewer MINOR #8**：CI 修补必须独立 commit，否则漏掉导致 CI 红

- [ ] **Step 1: 找到 CI workflow 文件**

Run: `ls .github/workflows/`
预期：看到 `test.yml` 或类似。

- [ ] **Step 2: 在 Linux runner 的 apt install 步骤加包**

找到形如：
```yaml
- name: Install system deps
  run: sudo apt-get install -y com0com
```

改为：
```yaml
- name: Install system deps
  run: sudo apt-get install -y com0com libfontconfig1-dev
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/
git commit -m "ci: 加 libfontconfig1-dev（font-kit 0.14 编译依赖）"
```

---

### Task 2: fonts.rs + list_mono_fonts

**Files:**
- Create: `src-tauri/src/fonts.rs`
- Modify: `src-tauri/src/lib.rs`（注册 `pub mod fonts`）
- Test: `src-tauri/src/fonts.rs`（同文件 `#[cfg(test)]` 模块）

> **Reviewer CRITICAL #2**：plan 之前给的两版实现（`Fs::all_fonts()` + `SystemSource::all_families()`）互相冲突。**只用 `SystemSource::all_families()` 一版**。
> **Reviewer 提示 #5**：去重 + sort（`BTreeSet` 天然去重 + 隐式排序）
> **Reviewer MAJOR #4**：补 `handles_empty_gracefully` 测试

- [ ] **Step 1: 写测试（红）**

新建 `src-tauri/src/fonts.rs`：

```rust
//! 跨平台系统等宽字体扫描
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
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `cd src-tauri && cargo test --lib fonts::tests 2>&1 | tail -10`
预期：`use font_kit::source::SystemSource` 编译失败（mod 没注册）。或 `unresolved import font_kit`。

- [ ] **Step 3: 在 lib.rs 注册 pub mod fonts**

`src-tauri/src/lib.rs` 顶部加：

```rust
pub mod fonts;
```

（用 `pub mod` 是为了让 `tests/` 集成测试能 `use crate::fonts::list_mono_fonts`）

- [ ] **Step 4: 跑测试，预期 PASS（绿）**

Run: `cd src-tauri && cargo test --lib fonts::tests 2>&1 | tail -10`
预期：4 个测试全过。

> **如果 `SystemSource::all_families` 实际签名不一致**（font-kit 0.14 内部 API 有微调），按编译器报错调整。常见替代：`source.all_fonts()` 返回 `Vec<Arc<Font>>`，每个 Font 调 `.family_name()`。**以 docs.rs/font-kit/0.14 为准**。

- [ ] **Step 5: Commit**

```bash
cd src-tauri
git add src/fonts.rs src/lib.rs
git commit -m "feat(rust): list_mono_fonts 跨平台扫描（去重 + 排序）"
```

---

### Task 3: AppConfig 加 font_size / font_family 字段

**Files:**
- Modify: `src-tauri/src/config_impl.rs:15-67`
- Test: `src-tauri/src/config_impl.rs`（同文件 tests 模块）

- [ ] **Step 1: 写测试（红）**

在 `src-tauri/src/config_impl.rs` 现有 `#[cfg(test)] mod tests` 段加 3 个测试：

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

在文件顶部 `// CONFIG_VERSION = 1` 附近加兜底函数：

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
预期：原测试 + 3 个新测试 = 全过。

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
//! Tauri command 不能脱离 Tauri runtime 直接调，简化测 IPC 包装层的序列化逻辑。

use oh_my_serial_lib::fonts::{list_mono_fonts, FontInfo};

#[test]
fn list_mono_fonts_serializes_to_valid_json() {
    let fonts: Vec<FontInfo> = list_mono_fonts();
    let json = serde_json::to_string(&fonts).expect("serialize");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("parse");
    for f in &parsed {
        assert!(f.get("family").is_some(), "每项必须有 family 字段");
        assert!(f["family"].is_string(), "family 必须是字符串");
    }
}
```

> **注意**：`oh_my_serial_lib` 是 lib crate 名字（看 `src-tauri/Cargo.toml:2` 的 `[lib] name` 或 `package.name`），如果实际是 `oh_my_serial` 就用 `oh_my_serial::fonts::*`。**`pub use fonts::*` 也要在 lib.rs 加**（看 Step 3）。

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `cd src-tauri && cargo test --test test_cmd_list_fonts 2>&1 | tail -10`
预期：编译错误 `unresolved import` 或 `private module`。

- [ ] **Step 3: 暴露 fonts API**

`src-tauri/src/lib.rs` 顶部加（如已有 `pub use` 段就加进去）：

```rust
pub use fonts::{list_mono_fonts, FontInfo};
```

- [ ] **Step 4: 加 cmd_list_fonts command**

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

- [ ] **Step 7: 跑完整 Rust 测试套件**

Run: `cd src-tauri && cargo test 2>&1 | tail -5`
预期：原 40 + 新 4（fonts）+ 3（config）+ 1（integration）= ~48 全过。

- [ ] **Step 8: Commit**

```bash
cd src-tauri
git add src/lib.rs src/ipc/commands.rs tests/test_cmd_list_fonts.rs
git commit -m "feat(rust): cmd_list_fonts IPC（列出系统等宽字体）"
```

---

## 阶段 2：前端基础（6 任务）

### Task 5: utils/fonts.ts 常量 + 纯函数

**Files:**
- Create: `src/utils/fonts.ts`
- Test: `src/utils/__tests__/fonts.test.ts`

- [ ] **Step 1: 写测试（红）**

新建 `src/utils/__tests__/fonts.test.ts`：

```ts
import { describe, it, expect } from "vitest";
import {
  clampFontSize,
  resolveFontFamily,
  FONT_SIZE_RANGE,
  SYSTEM_DEFAULT_FAMILY,
  SYSTEM_DEFAULT_KEY,
} from "../fonts";

describe("clampFontSize", () => {
  it("11 → 12 (下界)", () => expect(clampFontSize(11)).toBe(12));
  it("25 → 24 (上界)", () => expect(clampFontSize(25)).toBe(24));
  it("14 不变", () => expect(clampFontSize(14)).toBe(14));
  it("12 边界不变", () => expect(clampFontSize(12)).toBe(12));
  it("24 边界不变", () => expect(clampFontSize(24)).toBe(24));
});

describe("resolveFontFamily", () => {
  it("'system-default' 返回 fallback 常量", () => {
    expect(resolveFontFamily("system-default")).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("空串当作 system-default", () => {
    expect(resolveFontFamily("")).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("null 当作 system-default", () => {
    expect(resolveFontFamily(null)).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("普通字体名拼接 fallback 栈", () => {
    const result = resolveFontFamily("JetBrains Mono");
    expect(result).toContain("JetBrains Mono");
    expect(result).toContain(SYSTEM_DEFAULT_FAMILY);
  });
});

describe("FONT_SIZE_RANGE", () => {
  it("默认 14, 范围 12-24, 步进 2", () => {
    expect(FONT_SIZE_RANGE).toEqual({ min: 12, max: 24, step: 2, default: 14 });
  });
});

describe("SYSTEM_DEFAULT_KEY", () => {
  it("等于 'system-default'", () => {
    expect(SYSTEM_DEFAULT_KEY).toBe("system-default");
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
 * 与 Rust 端 AppConfig.font_size / AppConfig.font_family 一一对应
 */

/** xterm 默认字体栈（跨平台兜底） */
export const SYSTEM_DEFAULT_FAMILY = "Consolas, Monaco, 'Courier New', monospace";

/** 字号保留字（与 Rust 端 default_font_family() 一致） */
export const SYSTEM_DEFAULT_KEY = "system-default";

/** 字号范围与默认值 */
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

/** clamp 字号到合法范围 */
export function clampFontSize(n: number): number {
  return Math.max(FONT_SIZE_RANGE.min, Math.min(FONT_SIZE_RANGE.max, n));
}

/**
 * 解析 store 里的字体字段为 xterm 可用的 fontFamily 字符串
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

Run: `npx vitest run src/utils/__tests__/fonts.test.ts 2>&1 | tail -10`
预期：12 个测试全过。

- [ ] **Step 5: Commit**

```bash
git add src/utils/fonts.ts src/utils/__tests__/fonts.test.ts
git commit -m "feat(fe): 字体/字号常量 + 纯函数（12 单测）"
```

---

### Task 6: fontStore.ts

**Files:**
- Create: `src/stores/fontStore.ts`

- [ ] **Step 1: 实现 fontStore**

新建 `src/stores/fontStore.ts`：

```ts
/**
 * 字体列表缓存（不持久化，OS 级数据）
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

export const useFontStore = create<FontState>((set, get) => ({
  fonts: [],
  loaded: false,
  loadFonts: async () => {
    if (get().loaded) return; // 防御：避免重复 invoke
    try {
      const fonts = await invoke<FontInfo[]>("cmd_list_fonts");
      set({ fonts, loaded: true });
    } catch (e) {
      console.warn("[fontStore] 加载字体列表失败：", e);
      set({ fonts: [], loaded: true }); // loaded=true 避免反复重试
    }
  },
}));
```

- [ ] **Step 2: tsc 验证**

Run: `npx tsc --noEmit 2>&1 | tail -5`
预期：无 error。

- [ ] **Step 3: Commit**

```bash
git add src/stores/fontStore.ts
git commit -m "feat(fe): fontStore 缓存系统等宽字体列表"
```

---

### Task 7: configStore 加 setFontSize / setFontFamily（reviewer CRITICAL #1 修复）

**Files:**
- Modify: `src/stores/configStore.ts:19-30`（AppConfigFE interface）
- Modify: `src/stores/configStore.ts:32-43`（DEFAULT_CONFIG）
- Modify: `src/stores/configStore.ts:45-54`（ConfigState interface）
- Modify: `src/stores/configStore.ts:74-118`（impl）
- Test: `src/stores/__tests__/configStoreFont.test.ts`

> **关键修复（reviewer CRITICAL #1）**：
> - `configStore.config` 是嵌套 `AppConfigFE` 对象，**所有字段 snake_case**（与 Rust 一致）
> - **store 内加 2 个 setter**（与现有 `loadFromBackend` / `save` 并列）
> - setter 内置 clamp + trim 防御
> - 命名：`font_size` / `font_family`（**注意下划线**，与 Rust 透传）

- [ ] **Step 1: 写测试（红）**

新建 `src/stores/__tests__/configStoreFont.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useConfigStore, DEFAULT_CONFIG } from "../configStore";

describe("configStore font actions (CRITICAL #1 修复)", () => {
  beforeEach(() => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        font_size: 14,
        font_family: "system-default",
      },
    });
  });

  it("DEFAULT_CONFIG 默认值正确", () => {
    expect(DEFAULT_CONFIG.font_size).toBe(14);
    expect(DEFAULT_CONFIG.font_family).toBe("system-default");
  });

  it("setFontSize(20) 正常设置", () => {
    useConfigStore.getState().setFontSize(20);
    expect(useConfigStore.getState().config.font_size).toBe(20);
  });

  it("setFontSize(100) clamp 到 24", () => {
    useConfigStore.getState().setFontSize(100);
    expect(useConfigStore.getState().config.font_size).toBe(24);
  });

  it("setFontSize(0) clamp 到 12", () => {
    useConfigStore.getState().setFontSize(0);
    expect(useConfigStore.getState().config.font_size).toBe(12);
  });

  it("setFontFamily('') fallback 到 'system-default'", () => {
    useConfigStore.getState().setFontFamily("");
    expect(useConfigStore.getState().config.font_family).toBe("system-default");
  });

  it("setFontFamily('  JetBrains  ') trim 空格", () => {
    useConfigStore.getState().setFontFamily("  JetBrains  ");
    expect(useConfigStore.getState().config.font_family).toBe("JetBrains");
  });

  it("setFontSize 不影响其他字段", () => {
    const before = { ...useConfigStore.getState().config };
    useConfigStore.getState().setFontSize(20);
    const after = useConfigStore.getState().config;
    expect(after.baud_rate).toBe(before.baud_rate);
    expect(after.theme).toBe(before.theme);
    expect(after.font_family).toBe(before.font_family);
    expect(after.font_size).toBe(20);
  });
});
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `npx vitest run src/stores/__tests__/configStoreFont.test.ts 2>&1 | tail -10`
预期：`setFontSize is not a function` + `DEFAULT_CONFIG.font_size is undefined`。

- [ ] **Step 3: 改 AppConfigFE interface（绿）**

`src/stores/configStore.ts:19-30`，在 `reconnect_max_attempts: number;` 之后加：

```ts
  font_size: number;
  font_family: string;
```

- [ ] **Step 4: 改 DEFAULT_CONFIG（绿）**

`src/stores/configStore.ts:32-43`，在 `reconnect_max_attempts: 5,` 之后加：

```ts
  font_size: 14,
  font_family: "system-default",
```

- [ ] **Step 5: 改 ConfigState interface（绿）**

`src/stores/configStore.ts:45-54`，在 `save: () => Promise<void>;` 之后加：

```ts
  setFontSize: (n: number) => void;
  setFontFamily: (name: string) => void;
```

- [ ] **Step 6: 在 impl 加 setter（绿）**

`src/stores/configStore.ts:74-118`（`create<ConfigState>((set, get) => ({...}))`），在 `save: async () => {...},` 之后加：

```ts
setFontSize: (n: number) => {
  const clamped = Math.max(12, Math.min(24, n));
  set((state) => ({ config: { ...state.config, font_size: clamped } }));
},
setFontFamily: (name: string) => {
  const trimmed = name.trim() || "system-default";
  set((state) => ({ config: { ...state.config, font_family: trimmed } }));
},
```

> **关键**：用 `set((state) => ({ config: { ...state.config, ... } }))` 保留所有其他字段。**不能**用 `set({ font_size: ... })`（顶层没这字段）。

- [ ] **Step 7: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/stores/__tests__/configStoreFont.test.ts 2>&1 | tail -10`
预期：7 个测试全过。

- [ ] **Step 8: 跑完整前端测试套件**

Run: `npm test 2>&1 | tail -5`
预期：原 117 + 7 = 124 全过。

- [ ] **Step 9: Commit**

```bash
git add src/stores/configStore.ts src/stores/__tests__/configStoreFont.test.ts
git commit -m "feat(fe): configStore 加 setFontSize/setFontFamily（含 clamp + trim）"
```

---

### Task 8: useFontSize hook + main.tsx 防 FOUC

**Files:**
- Create: `src/hooks/useFontSize.ts`
- Modify: `src/main.tsx`
- Test: `src/hooks/__tests__/useFontSize.test.ts`

> **Reviewer 提示 #2**：`<html>` fontSize 必须在 main.tsx 入口**同步**设，否则首屏 FOUC。

- [ ] **Step 1: 写 useFontSize hook**

新建 `src/hooks/useFontSize.ts`：

```ts
/**
 * 同步 configStore.config.font_size 到 <html> style.fontSize
 * - 启动时由 main.tsx 显式同步调用一次（防 FOUC）
 * - 之后随 store 变化自动同步
 */
import { useEffect } from "react";
import { useConfigStore, DEFAULT_CONFIG } from "../stores/configStore";

export function useFontSize(): void {
  const fontSize = useConfigStore((s) => s.config.font_size);

  useEffect(() => {
    document.documentElement.style.fontSize = `${fontSize}px`;
  }, [fontSize]);
}

/** 同步应用字号（不进 React 生命周期，main.tsx 启动时同步调用） */
export function applyFontSizeSync(fontSize: number = DEFAULT_CONFIG.font_size): void {
  document.documentElement.style.fontSize = `${fontSize}px`;
}
```

- [ ] **Step 2: 写测试（红）**

新建 `src/hooks/__tests__/useFontSize.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useFontSize, applyFontSizeSync } from "../useFontSize";
import { useConfigStore, DEFAULT_CONFIG } from "../../stores/configStore";

describe("useFontSize", () => {
  beforeEach(() => {
    document.documentElement.style.fontSize = "";
    useConfigStore.setState({ config: { ...DEFAULT_CONFIG, font_size: 14 } });
  });

  it("初始 mount 时同步 store.config.font_size 到 <html>", () => {
    useConfigStore.setState({ config: { ...DEFAULT_CONFIG, font_size: 18 } });
    renderHook(() => useFontSize());
    expect(document.documentElement.style.fontSize).toBe("18px");
  });

  it("store.config.font_size 变化时同步更新", () => {
    useConfigStore.setState({ config: { ...DEFAULT_CONFIG, font_size: 14 } });
    const { rerender } = renderHook(() => useFontSize());
    expect(document.documentElement.style.fontSize).toBe("14px");
    useConfigStore.setState({ config: { ...DEFAULT_CONFIG, font_size: 20 } });
    rerender();
    expect(document.documentElement.style.fontSize).toBe("20px");
  });
});

describe("applyFontSizeSync", () => {
  beforeEach(() => {
    document.documentElement.style.fontSize = "";
  });

  it("默认参数同步设置 <html> fontSize 为 14", () => {
    applyFontSizeSync();
    expect(document.documentElement.style.fontSize).toBe("14px");
  });

  it("显式传参同步设置", () => {
    applyFontSizeSync(20);
    expect(document.documentElement.style.fontSize).toBe("20px");
  });
});
```

- [ ] **Step 3: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/hooks/__tests__/useFontSize.test.ts 2>&1 | tail -10`
预期：4 个测试过。

- [ ] **Step 4: 改 main.tsx 防 FOUC**

`src/main.tsx` 在 `createRoot(...).render(...)` 之前插入：

```ts
import { applyFontSizeSync } from "./hooks/useFontSize";

// 启动时同步应用字号（防 FOUC）—— 必须在 React 首次 commit 之前
applyFontSizeSync();
```

- [ ] **Step 5: tsc 验证**

Run: `npx tsc --noEmit 2>&1 | tail -5`
预期：无 error。

- [ ] **Step 6: Commit**

```bash
git add src/hooks/useFontSize.ts src/hooks/__tests__/useFontSize.test.ts src/main.tsx
git commit -m "feat(fe): useFontSize hook + main.tsx 同步防 FOUC"
```

---

### Task 9: useConfigSync 改造（reviewer MAJOR #7 完整代码）

**Files:**
- Modify: `src/hooks/useConfigSync.ts:53-67`（sync 函数保留 2 字段）
- Modify: `src/hooks/useConfigSync.ts:116-124`（加 configStore 订阅）
- Test: `src/hooks/__tests__/useConfigSync.test.ts`

> **关键修复（reviewer MAJOR #7）**：
> 1. `sync()` 函数**保留** `font_size` / `font_family`（从 store 读 → 写回），避免 sync 覆盖用户改的值
> 2. **新增 configStore 订阅**（监听 font_size/font_family 变化）→ 触发 `scheduleSave()`，但**不调** `sync()`（避免递归）
> 3. `loaded` 守卫：loadFromBackend 完成后才能 scheduleSave

- [ ] **Step 1: 写测试（红）**

新建 `src/hooks/__tests__/useConfigSync.test.ts`：

```ts
import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useConfigSync } from "../useConfigSync";
import { useConfigStore, DEFAULT_CONFIG } from "../../stores/configStore";

// mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

import { invoke } from "@tauri-apps/api/core";

describe("useConfigSync font 集成 (MAJOR #7 修复)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_size: 14, font_family: "system-default" },
      loaded: true, // 跳过 loadFromBackend 异步
      loading: false,
    });
    vi.mocked(invoke).mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("setFontSize(20) → 500ms 后 invoke('cmd_save_config') 被调", () => {
    renderHook(() => useConfigSync());
    useConfigStore.getState().setFontSize(20);
    expect(invoke).not.toHaveBeenCalled(); // 500ms 内不调
    vi.advanceTimersByTime(500);
    expect(invoke).toHaveBeenCalledWith(
      "cmd_save_config",
      expect.objectContaining({
        config: expect.objectContaining({ font_size: 20 }),
      })
    );
  });

  it("setFontSize 多次连改 → debounce 500ms 后只调 1 次，写最终值", () => {
    renderHook(() => useConfigSync());
    useConfigStore.getState().setFontSize(16);
    vi.advanceTimersByTime(200);
    useConfigStore.getState().setFontSize(18);
    vi.advanceTimersByTime(200);
    useConfigStore.getState().setFontSize(20); // 最终值
    vi.advanceTimersByTime(500);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(
      "cmd_save_config",
      expect.objectContaining({
        config: expect.objectContaining({ font_size: 20 }),
      })
    );
  });

  it("sync() 不覆盖 font_size / font_family（关键修复）", () => {
    renderHook(() => useConfigSync());
    // 用户改字号
    useConfigStore.getState().setFontSize(20);
    // 模拟 serial store 触发 sync（看 useConfigSync sync 模式）
    useConfigStore.getState().setBaudRate(9600); // 这会触发 unsubSerial 里的 sync
    // font_size 仍应是 20（sync 没覆盖）
    expect(useConfigStore.getState().config.font_size).toBe(20);
  });
});
```

- [ ] **Step 2: 跑测试，预期 FAIL（红）**

Run: `npx vitest run src/hooks/__tests__/useConfigSync.test.ts 2>&1 | tail -10`
预期：font_size 字段不存在，sync 不保留。

- [ ] **Step 3: 改 sync() 保留 2 字段**

`src/hooks/useConfigSync.ts:53-67`，在 `setState` 的 `config: {...}` 字面量末尾加：

```ts
      useConfigStore.setState({
        config: {
          last_port: serial.portName || null,
          baud_rate: serial.baudRate,
          data_bits: serial.dataBits,
          stop_bits: serial.stopBits,
          parity: serial.parity,
          encoding: serial.encoding,
          theme: ui.theme,
          buffer_size: validBuffer,
          auto_reconnect: useConfigStore.getState().config.auto_reconnect,
          reconnect_max_attempts:
            useConfigStore.getState().config.reconnect_max_attempts,
          // ★ 新增：保留 font_size / font_family（不覆盖用户改的）
          font_size: useConfigStore.getState().config.font_size,
          font_family: useConfigStore.getState().config.font_family,
        },
      });
```

- [ ] **Step 4: 加 configStore 订阅触发 save（不调 sync）**

`src/hooks/useConfigSync.ts:124` 之后（`unsubUi` 之后）加：

```ts
    const unsubConfig = useConfigStore.subscribe(
      (s) => [s.config.font_size, s.config.font_family] as const,
      () => {
        // font_size/font_family 变化 → 仅 scheduleSave，不调 sync
        // （避免 sync 把 config 整个覆盖一次）
        if (useConfigStore.getState().loaded) {
          scheduleSave();
        }
      },
      {
        equalityFn: (a, b) => a[0] === b[0] && a[1] === b[1],
      },
    );
```

- [ ] **Step 5: 清理函数加 unsubConfig**

`src/hooks/useConfigSync.ts:126-133` 的 return 清理函数，加：

```ts
      unsubConfig();
```

在 `unsubBuffer();` 之后。

- [ ] **Step 6: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/hooks/__tests__/useConfigSync.test.ts 2>&1 | tail -10`
预期：3 个测试过。

- [ ] **Step 7: 跑完整前端测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：124 + 3 = 127 全过，tsc 无 error。

- [ ] **Step 8: Commit**

```bash
git add src/hooks/useConfigSync.ts src/hooks/__tests__/useConfigSync.test.ts
git commit -m "feat(fe): useConfigSync 加 font_size/font_family sync（保留字段 + debounce save）"
```

---

## 阶段 3：前端 UI（5 任务）

### Task 10: Terminal.tsx 接入（带 rAF 防阻塞）

**Files:**
- Modify: `src/components/Terminal.tsx:67-93`（初始化读 store）
- Modify: `src/components/Terminal.tsx:102-107`（加 useEffect 监听字号/字体）
- Test: `src/components/__tests__/TerminalFont.test.tsx`

> **Reviewer 提示 #1**：xterm 字号切换在 > 5k 行历史数据下会触发全量重绘，用 `requestAnimationFrame` 延迟到下一帧。
> **Reviewer MAJOR #5**：补 3 个 Terminal 测试

- [ ] **Step 1: 改 Terminal.tsx 读 store**

`src/components/Terminal.tsx` 顶部 import 加：

```ts
import { useConfigStore } from "../stores/configStore";
import { resolveFontFamily } from "../utils/fonts";
```

`src/components/Terminal.tsx:67-93` 的 `useEffect`（初始化）开头加：

```ts
  const fontSize = useConfigStore((s) => s.config.font_size);
  const fontFamily = useConfigStore((s) => s.config.font_family);

  useEffect(() => {
    if (!terminalContainerRef.current) return;
    const term = new Terminal({
      fontSize,
      fontFamily: resolveFontFamily(fontFamily),
      // ... 现有 options 保留
    });
    // ... 现有初始化逻辑保留
  }, []); // 保持 []，只在 mount 时初始化
```

- [ ] **Step 2: 加 useEffect 监听字号/字体变化（带 rAF）**

`src/components/Terminal.tsx:102-107` 之后加：

```ts
  useEffect(() => {
    if (!xtermRef.current) return;
    const rafId = requestAnimationFrame(() => {
      if (xtermRef.current) {
        xtermRef.current.options.fontSize = fontSize;
        // ★ reviewer 友情提示 #2：必须显式 refresh，否则 xterm 只更新 metrics
        // 不重排已渲染行 → 用户视觉上"字号变了但行高没变"会困惑
        xtermRef.current.refresh(0, xtermRef.current.rows - 1);
      }
    });
    return () => cancelAnimationFrame(rafId);
  }, [fontSize]);

  useEffect(() => {
    if (!xtermRef.current) return;
    const rafId = requestAnimationFrame(() => {
      if (xtermRef.current) {
        xtermRef.current.options.fontFamily = resolveFontFamily(fontFamily);
        xtermRef.current.refresh(0, xtermRef.current.rows - 1);
      }
    });
    return () => cancelAnimationFrame(rafId);
  }, [fontFamily]);
```

- [ ] **Step 3: 写 3 个组件测试（reviewer MAJOR #5）**

新建 `src/components/__tests__/TerminalFont.test.tsx`：

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { Terminal } from "../Terminal";
import { useConfigStore, DEFAULT_CONFIG } from "../../stores/configStore";
import { SYSTEM_DEFAULT_FAMILY } from "../../utils/fonts";

// mock xterm
const mockTerminalInstance = {
  options: {} as any,
  open: vi.fn(),
  write: vi.fn(),
  dispose: vi.fn(),
  reset: vi.fn(),
};
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => mockTerminalInstance),
}));

describe("Terminal 字号/字体更新 (MAJOR #5)", () => {
  beforeEach(() => {
    mockTerminalInstance.options = {};
    vi.clearAllMocks();
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_size: 14, font_family: "system-default" },
    });
  });

  it("fontSize 变化时 xterm.options.fontSize 被更新（rAF 后）", async () => {
    const { rerender } = render(<Terminal />);
    useConfigStore.setState({ config: { ...DEFAULT_CONFIG, font_size: 20 } });
    rerender(<Terminal />);
    await waitFor(() => {
      expect(mockTerminalInstance.options.fontSize).toBe(20);
    });
  });

  it("fontFamily 变化时 xterm.options.fontFamily 拼接 fallback 栈", async () => {
    const { rerender } = render(<Terminal />);
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_family: "JetBrains Mono" },
    });
    rerender(<Terminal />);
    await waitFor(() => {
      expect(mockTerminalInstance.options.fontFamily).toContain("JetBrains Mono");
      expect(mockTerminalInstance.options.fontFamily).toContain(SYSTEM_DEFAULT_FAMILY);
    });
  });

  it("fontFamily === 'system-default' 走 fallback 常量（无前缀拼接）", async () => {
    const { rerender } = render(<Terminal />);
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_family: "JetBrains Mono" },
    });
    rerender(<Terminal />);
    await waitFor(() => {
      // 切到 system-default
      useConfigStore.setState({
        config: { ...DEFAULT_CONFIG, font_family: "system-default" },
      });
    });
    await waitFor(() => {
      expect(mockTerminalInstance.options.fontFamily).toBe(SYSTEM_DEFAULT_FAMILY);
    });
  });
});
```

- [ ] **Step 4: 跑测试，预期 PASS（绿）**

Run: `npx vitest run src/components/__tests__/TerminalFont.test.tsx 2>&1 | tail -10`
预期：3 个测试过。

- [ ] **Step 5: tsc 验证**

Run: `npx tsc --noEmit 2>&1 | tail -5`
预期：无 error。

- [ ] **Step 6: Commit**

```bash
git add src/components/Terminal.tsx src/components/__tests__/TerminalFont.test.tsx
git commit -m "feat(fe): Terminal 接入字号/字体（rAF 防阻塞 + 3 单测）"
```

---

### Task 11: FontPicker Combobox（含 a11y 焦点 + 键盘 ↑↓）

**Files:**
- Create: `src/components/FontPicker.tsx`
- Test: `src/components/__tests__/FontPicker.test.tsx`

> **Reviewer 提示 #4**：打开后自动 focus input，关闭后焦点回到触发按钮
> **Reviewer MAJOR #6**：补键盘 ↑↓ Enter 移动高亮测试

- [ ] **Step 1: 写 11 个组件测试（红）**

新建 `src/components/__tests__/FontPicker.test.tsx`：

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { FontPicker } from "../FontPicker";
import { useFontStore } from "../../stores/fontStore";
import { useConfigStore, DEFAULT_CONFIG } from "../../stores/configStore";

describe("FontPicker Combobox (MAJOR #6 补全)", () => {
  beforeEach(() => {
    useFontStore.setState({
      fonts: [
        { family: "Consolas" },
        { family: "JetBrains Mono" },
        { family: "Cascadia Code" },
      ],
      loaded: true,
    });
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_family: "system-default" },
    });
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
    fireEvent.click(screen.getByText("JetBrains Mono"));
    expect(useConfigStore.getState().config.font_family).toBe("JetBrains Mono");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("Esc 关闭列表", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("打开后 input 自动 focus（reviewer #4）", async () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    await new Promise((r) => requestAnimationFrame(r));
    expect(screen.getByRole("textbox")).toHaveFocus();
  });

  it("关闭后焦点回到触发按钮（reviewer #4）", async () => {
    render(<FontPicker />);
    const trigger = screen.getByRole("button");
    fireEvent.click(trigger);
    await new Promise((r) => requestAnimationFrame(r));
    fireEvent.keyDown(document, { key: "Escape" });
    await new Promise((r) => requestAnimationFrame(r));
    expect(trigger).toHaveFocus();
  });

  it("ArrowDown 移动高亮到第 2 项（MAJOR #6）", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    // 第 1 项是"系统默认"，第 2 项是"Consolas"
    const items = screen.getAllByRole("option");
    expect(items[0].className).toContain("bg-blue-50"); // 初始高亮第 0 项
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(items[1].className).toContain("bg-blue-50");
  });

  it("ArrowUp 在第 1 项时不变（MAJOR #6 边界）", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    const items = screen.getAllByRole("option");
    expect(items[0].className).toContain("bg-blue-50");
    fireEvent.keyDown(document, { key: "ArrowUp" });
    expect(items[0].className).toContain("bg-blue-50");
  });

  it("Enter 选中当前高亮项（MAJOR #6）", () => {
    render(<FontPicker />);
    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(document, { key: "ArrowDown" }); // 高亮移到 Consolas
    fireEvent.keyDown(document, { key: "Enter" });
    expect(useConfigStore.getState().config.font_family).toBe("Consolas");
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

新建 `src/components/FontPicker.tsx`（~140 行）：

```tsx
/**
 * 字体 Combobox
 * - 触发按钮显示当前字体
 * - 打开：input + ul 列表
 * - 第 1 项固定"系统默认（当前）" + 分隔线 + 字体列表
 * - a11y：打开 focus input / 关闭焦点回触发按钮
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
  const fontFamily = useConfigStore((s) => s.config.font_family);
  const setFontFamily = useConfigStore((s) => s.setFontFamily);

  // 第 1 项"系统默认" + 字体列表
  const items = [
    { family: SYSTEM_DEFAULT_KEY, label: "系统默认" },
    ...fonts.map((f) => ({ family: f.family, label: f.family })),
  ];

  const filtered = filter
    ? items.filter((i) => i.label.toLowerCase().includes(filter.toLowerCase()))
    : items;

  // 重置高亮当 filter 变时
  useEffect(() => {
    setHighlight(0);
  }, [filter, open]);

  // 打开/关闭 a11y 焦点管理
  useEffect(() => {
    if (open) {
      // rAF 等 React commit 完，避免 useEffect 竞争条件
      const raf = requestAnimationFrame(() => inputRef.current?.focus());
      return () => cancelAnimationFrame(raf);
    } else if (triggerRef.current) {
      // 关闭后焦点回到触发按钮
      const raf = requestAnimationFrame(() => triggerRef.current?.focus());
      return () => cancelAnimationFrame(raf);
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

  // 键盘交互：Esc / ↑ / ↓ / Enter
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      // ★ reviewer 友情提示 #5：中文输入法拼音期间不响应键盘导航
      if (e.isComposing) return;
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
              // ★ reviewer 友情提示 #5：中文输入法拼音期间不重置 highlight
              if (e.nativeEvent.isComposing) {
                setFilter(e.target.value);
                return;
              }
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
预期：11 个测试全过。

- [ ] **Step 5: Commit**

```bash
git add src/components/FontPicker.tsx src/components/__tests__/FontPicker.test.tsx
git commit -m "feat(fe): FontPicker Combobox（a11y 焦点 + 键盘 ↑↓ Enter Esc）"
```

---

### Task 12: SerialToolbar 步进按钮 + 字体容器

**Files:**
- Modify: `src/components/SerialToolbar.tsx`

- [ ] **Step 1: 改 SerialToolbar**

import 段加：

```ts
import { useConfigStore } from "../stores/configStore";
import { FONT_SIZE_RANGE } from "../utils/fonts";
import { FontPicker } from "./FontPicker";
```

组件内（在主题 select 之后的位置）加：

```tsx
  const fontSize = useConfigStore((s) => s.config.font_size);
  const setFontSize = useConfigStore((s) => s.setFontSize);
```

JSX（在主题 select 之后）插入：

```tsx
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
预期：127 + 0 = 127 全过，tsc 无 error。

- [ ] **Step 3: Commit**

```bash
git add src/components/SerialToolbar.tsx
git commit -m "feat(fe): SerialToolbar 加步进按钮组 + 字体容器"
```

---

### Task 13: App.tsx 3 个 hotkey

**Files:**
- Modify: `src/App.tsx:101-132`

> **Reviewer MINOR #9**：`Ctrl++` 在键盘上是 `Ctrl+Shift+=`，但 `matchHotkey` 不区分大小写，key 字段写 `=` 即可匹配。

- [ ] **Step 1: 加 hotkey**

import 段加：

```ts
import { useConfigStore } from "./stores/configStore";
import { FONT_SIZE_RANGE } from "./utils/fonts";
```

在现有 hotkey 数组后加 3 项：

```ts
    { key: "=", ctrl: true, handler: () => useConfigStore.getState().setFontSize(useConfigStore.getState().config.font_size + FONT_SIZE_RANGE.step), description: "增大字号" },
    { key: "-", ctrl: true, handler: () => useConfigStore.getState().setFontSize(useConfigStore.getState().config.font_size - FONT_SIZE_RANGE.step), description: "减小字号" },
    { key: "0", ctrl: true, handler: () => useConfigStore.getState().setFontSize(FONT_SIZE_RANGE.default), description: "重置字号" },
```

- [ ] **Step 2: 跑测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：127 全过，tsc 无 error。

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat(fe): 加 3 个字号 hotkey (Ctrl++/-/0，实际 chord 是 Ctrl+Shift+=/Ctrl+-/Ctrl+0)"
```

---

### Task 14: HotkeyHelp "显示"分组

**Files:**
- Modify: `src/components/HotkeyHelp.tsx:54-66`

- [ ] **Step 1: 改 HotkeyHelp 表格结构**

把现有 hotkeys 数组（如果不在 HotkeyHelp.tsx 内部而是从 App.tsx 传过来，需要从 App.tsx 拿）按分组：

```tsx
const groups = [
  { title: "通用", keys: ["清空终端", "聚焦发送", "主题切换", "日志面板"] },
  { title: "显示", keys: ["增大字号", "减小字号", "重置字号"] },
];

// 渲染时分两个 <tbody> 或两段，加分组标题 <h3>
```

具体写法按 HotkeyHelp.tsx 现有模板（看实际代码调整）。

- [ ] **Step 2: 跑测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：127 全过，tsc 无 error。

- [ ] **Step 3: Commit**

```bash
git add src/components/HotkeyHelp.tsx
git commit -m "feat(fe): HotkeyHelp 加'显示'分组（字号快捷键）"
```

---

## 阶段 4：集成 & 验证（3 任务）

### Task 14.5: 硬编码 px 修复（条件性，前置依赖 0.3 触发）

> **Reviewer MAJOR #3**：本 Task 只在 0.3 grep 审计**发现 ≥ 1 处需要改**时执行。

- [ ] **Step 1: 跑 grep 审计（前置依赖 0.3）**

Run: `cd .. && grep -rnE "(\bw-\[|h-\[|min-h-\[|p-\[|m-\[|gap-\[)" src/ | head -30`

- [ ] **Step 2: 分类每条命中（A 溢出风险 / B 高度不足 / C 间距紧 / D 忽略）**

按 0.3 描述的方法分类。

- [ ] **Step 3: 改 A/B/C 命中项**

举例（按实际命中调整）：
- `w-[200px]` → `w-[10rem]`（按钮宽度跟字号）
- `min-h-8` → `min-h-[2rem]`
- 浮层宽度（如 `HotkeyHelp.tsx:50`）→ 保留 `px`（D 忽略，不跟字号缩放）

- [ ] **Step 4: 跑前端测试 + tsc**

Run: `npm test 2>&1 | tail -5 && npx tsc --noEmit 2>&1 | tail -5`
预期：127 全过，tsc 无 error。

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "fix(ui): 硬编码 px 尺寸 rem 化（24px 字号下不溢出）"
```

---

### Task 15: 回归测试（127 + 新增）

- [ ] **Step 1: 跑 Rust 完整测试**

Run: `cd src-tauri && cargo test 2>&1 | tail -10`
预期：原 40 + 新 4（fonts）+ 3（config）+ 1（integration）= ~48 全过。

- [ ] **Step 2: 跑前端完整测试**

Run: `cd .. && npm test 2>&1 | tail -10`
预期：原 117 + 12（utils/fonts）+ 7（configStore font）+ 4（useFontSize）+ 3（useConfigSync）+ 3（Terminal）+ 11（FontPicker）= ~157 全过。

- [ ] **Step 3: 跑 tsc**

Run: `npx tsc --noEmit 2>&1 | tail -5`
预期：无 error。

- [ ] **Step 4: 跑 cargo clippy**

Run: `cd src-tauri && cargo clippy -- -D warnings 2>&1 | tail -5`
预期：无 warning。

- [ ] **Step 5: 若有失败**回到对应 Task 修，**不 commit 这一步**。

---

### Task 16: 手动验证 20 项（`tauri dev`）

按 spec §6.3 跑 20 项手动验证清单：

**基础流（10）**：

- [ ] **1.** 启动 → 步进按钮显示 "14px"，全 UI 默认 14px
- [ ] **2.** 点 A+ → 16px → 全 UI 同步放大
- [ ] **3.** 连续点 A+ → 18/20/22/24 → 第 5 次按钮 disabled
- [ ] **4.** 点 A− → 22px → 反向步进
- [ ] **5.** 点中间"24px" → 重置 14px
- [ ] **6.** Ctrl++ → 16px（步进 2）
- [ ] **7.** Ctrl+- → 14px
- [ ] **8.** Ctrl+0 → 14px（无论当前值）
- [ ] **9.** 字号 ≤ 12 / ≥ 24 时按钮变灰
- [ ] **10.** 字号 clamp 在快捷键下也生效

**字体流（6）**：

- [ ] **11.** 字体 Combobox 列出 ≥ 10 个等宽字体
- [ ] **12.** 输入"jet" → 列表过滤为 JetBrains Mono
- [ ] **13.** 选 JetBrains Mono → 终端字体立即切换
- [ ] **14.** 输入不存在关键字"xyz" → "无匹配"
- [ ] **15.** 选"系统默认" → 终端用 Consolas/Monaco 栈
- [ ] **16.** 字体 Combobox Esc / 点击外部 → 关闭 + 焦点回到触发按钮

**持久化（2）**：

- [ ] **17.** 改字号 → 等 500ms → 关 app → 重开 → 字号保留
- [ ] **18.** 改字体 → 重开 → 字体保留

**布局（2）**：

- [ ] **19.** 24px 字号下：SerialToolbar / StatusBar / SendPanel / LogPanel 无溢出/挤压
- [ ] **20.** 12px 字号下：上述组件文字可读

---

### Task 17: 文档更新

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`
- Create: `docs/releases/v1.1.0.md`

- [ ] **Step 1: 用 update-claudemd skill**

`/update-claudemd` 同步 CLAUDE.md（skill 触发见 `.claude/skills/update-claudemd/SKILL.md`）

- [ ] **Step 2: 用 update-readme skill**

`/update-readme` 同步 README.md

- [ ] **Step 3: 写 v1.1.0 release notes**

新建 `docs/releases/v1.1.0.md`，参考 `docs/releases/v1.0.2.md` 风格。

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md README.md docs/releases/v1.1.0.md
git commit -m "docs: v1.1.0 release（字体/字号切换）"
```

---

## 实施总览（commit 边界）

| # | Commit | 阶段 | 可独立停下 |
|---|--------|------|----------|
| 1 | `build(deps): 加 font-kit + 精简 tokio` | 1 | ✅ |
| 1.5 | `ci: 加 libfontconfig1-dev` | 1 | ✅（独立 release blocker） |
| 2 | `feat(rust): list_mono_fonts` | 1 | ✅ |
| 3 | `feat(rust): AppConfig 加 2 字段` | 1 | ✅ |
| 4 | `feat(rust): cmd_list_fonts IPC` | 1 | ✅ |
| 5 | `feat(fe): 字体/字号常量 + 纯函数` | 2 | ✅ |
| 6 | `feat(fe): fontStore` | 2 | ✅ |
| 7 | `feat(fe): configStore setFontSize/Family` | 2 | ✅ |
| 8 | `feat(fe): useFontSize + 防 FOUC` | 2 | ✅ |
| 9 | `feat(fe): useConfigSync font sync` | 2 | ✅ |
| 10 | `feat(fe): Terminal 接入字号/字体` | 3 | ✅ |
| 11 | `feat(fe): FontPicker Combobox` | 3 | ✅ |
| 12 | `feat(fe): SerialToolbar 步进按钮 + 字体容器` | 3 | ✅ |
| 13 | `feat(fe): 3 个字号 hotkey` | 3 | ✅ |
| 14 | `feat(fe): HotkeyHelp 显示分组` | 3 | ✅ |
| 14.5 | `fix(ui): 硬编码 px rem 化`（条件性） | 3 | ✅ |
| 15 | 验证（无 commit） | 4 | — |
| 16 | 手动验证（无 commit） | 4 | — |
| 17 | `docs: v1.1.0 release` | 4 | ✅ |

**总 commit 数：15-16 个**（条件性 Task 14.5 可选）

---

## Reviewer 提示落实位置

| 提示 | 落实位置 |
|------|---------|
| #1 xterm rAF 防阻塞 | Task 10 Step 2 |
| #2 main.tsx 防 FOUC | Task 8 Step 4 |
| #3 font-kit MSRV 预检 | 前置依赖 0.2 |
| #4 Combobox a11y 焦点 | Task 11 Step 3 useEffect + 2 测试 |
| #5 字体去重 + sort | Task 2 Step 3 BTreeSet |
| CRITICAL #1 configStore 嵌套 | Task 7 全文 + Task 8/9/10/11/12/13 全部改 config.font_size 路径 |
| CRITICAL #2 Task 2 Step 3 删占位 | Task 2 Step 3 单实现 |
| MAJOR #3 硬编码 px 审计 | 前置依赖 0.3 + Task 14.5 条件性 |
| MAJOR #4 handles_empty_gracefully | Task 2 Step 1 测试 |
| MAJOR #5 Terminal 3 测试 | Task 10 Step 3 |
| MAJOR #6 FontPicker 键盘 ↑↓ | Task 11 Step 1 + 3 测试 |
| MAJOR #7 useConfigSync 完整 | Task 9 Step 3-5 完整代码 |
| MINOR #8 CI commit | Task 1.5 独立 commit |
| MINOR #9 hotkey Ctrl++ | Task 13 Step 1 注释 |

---

## 风险与回退

- **font-kit 0.14 编译失败** → 改 `default-features = false, features = ["loader-fontconfig", "loader-freetype"]`
- **xterm rAF 后仍阻塞** → 退到 `setTimeout(fn, 0)`
- **Combobox 60 项卡顿** → 加 `useMemo` 缓存 filtered
- **硬编码 px 审计发现 ≥ 5 处需改** → Task 14.5 拆多个 commit 按组件分开改

---

## 不在本计划范围内

- ❌ 发布 v1.1.0 到 GitHub Releases（用 `/release` skill 单独走流程）
- ❌ 跨平台 CI 完整验证（Windows runner / macOS runner 由 GitHub Actions 跑）
- ❌ 性能基准（criterion bench 不适用于字体功能）

---

## 实施期检查清单（前置依赖 0.3 输出）

> **实施时填**：

```
grep 命中总数：____
A 类（溢出风险）：____ 处 → 改 ____
B 类（高度不足）：____ 处 → 改 ____
C 类（间距紧）：____ 处 → 改 ____
D 类（忽略）：____ 处 → 不改

是否需要执行 Task 14.5？[ ] 是 [ ] 否
```

---

## 关键文件:行号速查（实施时备查）

| 文件 | 关键位置 |
|------|---------|
| `src-tauri/src/fonts.rs` | 新建 |
| `src-tauri/src/config_impl.rs` | AppConfig struct line 15-38, Default impl line 51-67 |
| `src-tauri/src/ipc/commands.rs` | 末尾加 cmd_list_fonts |
| `src-tauri/src/lib.rs` | line 22-56 run(), 顶部加 `pub mod fonts` |
| `src/stores/configStore.ts` | AppConfigFE line 19-30, DEFAULT_CONFIG line 32-43, ConfigState line 45-54, impl line 74-118 |
| `src/hooks/useConfigSync.ts` | sync() line 42-73, 订阅 line 79-124 |
| `src/components/Terminal.tsx` | init useEffect line 67-93, theme useEffect line 102-107 |
| `src/components/SerialToolbar.tsx` | 主题 select 位置（line 137-147）后插步进按钮 |
| `src/components/HotkeyHelp.tsx` | table line 54-66 |
| `src/App.tsx` | hotkey array line 101-132 |
