# 字体/字号切换功能 — 设计文档

> **Status:** Draft (待 spec-document-reviewer 审核)
> **Date:** 2026-06-12
> **Target version:** v1.1.0（继 v1.0.2 之后）
> **Author:** 人类需求 + Claude 协助设计
> **Scope:** 新功能（非 bug 修复 / 非重构）

---

## 1. 背景与动机

### 1.1 现状

OhMySerial 截至 v1.0.2，**所有字体 / 字号设置都是硬编码**：

| 位置 | 硬编码值 | 文件:行 |
|------|---------|---------|
| 终端字号 | `14` | `src/components/Terminal.tsx:73` |
| 终端字体 | `"Consolas, Monaco, 'Courier New', monospace"` | `src/components/Terminal.tsx:74` |
| UI 全局字体 | 系统无衬线栈 | `src/index.css:24-30` |
| 各 UI 字号 | `text-xs` / `text-sm` / `text-lg` Tailwind class | 散落各组件 |

### 1.2 动机

- 工业控制现场环境多样：**低 DPI 屏需要大字号**（老花眼 / 远距离观察）、**高 DPI 屏需要小字号**（屏幕小但分辨率高，希望显示更多数据）
- 当前**没有任何方式调整字号**——用户提过 3 次以上
- xterm.js 内置的 `fontFamily` 选项支持运行时切换（已有 `theme` 切换的同模式）
- Rust `font-kit` crate 可跨平台列出系统已安装字体（**比 Web FontFace API 强**）

### 1.3 不做（YAGNI）

- ❌ **UI 字体可调**（用户确认"只给终端用"）—— 改 UI 字体可能让布局崩（中文/英文宽度差异）
- ❌ **自定义字体上传**——工业工具不需要
- ❌ **字号缩放时的"防爆"保护**——12-24 范围已经审计安全
- ❌ **撤销历史栈**——简单性优先
- ❌ **字体预览（hover 显示效果）**——UI 复杂度不值得
- ❌ **第三方 UI 库**（Headless UI / Radix）——Combobox 自己手搓

---

## 2. 设计决策总览

5 个澄清问题 + 1 个方案选择 = 6 个决策点：

| # | 维度 | 决策 | 备选 |
|---|------|------|------|
| 1 | 作用范围 | **B. 字号全局联动 + 字体只给终端** | A 只改 xterm / C 拆成两组独立 |
| 2 | 持久化 | **configStore（Rust AppConfig 镜像）** | uiStore（localStorage） |
| 3 | UI 入口 | **C. 步进按钮 + Ctrl+±/0 快捷键 + 字体 Combobox** | A 两个 select / B 步进无快捷键 / D 工具栏不放 |
| 4 | 字体范围 | **A+a. 只列等宽 + 只给终端** | B 完整 5 项 / C 6+自定义 / D 只 input |
| 5 | 字体 UI | **A. Combobox（input 过滤 + ul）** | B 原生 select / C 分组 |
| 6 | 实现方式 | **方案 2. 根 font-size 缩放（rem 化）** | 方案 1. CSS 变量 + 4 档 |

---

## 3. 架构

### 3.1 核心机制

通过改 `<html>` 的 `font-size`（px）→ 全部 `rem` 单位自动缩放 → **全 UI 联动**。终端字号独立走 `xterm.options.fontSize`。

**关键点**：根字号必须是**绝对单位（px）**，否则循环依赖。改 px 后所有 rem 才"知道"基准。

### 3.2 字号范围

| 项 | 值 |
|----|---|
| 范围 | 12 - 24 px |
| 步进 | 2 px（按钮）/ 2 px（快捷键） |
| 档数 | 7 档（12/14/16/18/20/22/24） |
| 默认 | 14 px |

### 3.3 字号档位表（rem 联动效果参考）

| 档位 | `<html>` font-size | text-sm (= 0.875rem) 实际像素 |
|------|---------------------|-------------------------------|
| 12 | 12px | 10.5px（偏小） |
| **14（默认）** | **14px** | **12.25px（标准）** |
| 16 | 16px | 14px（偏大） |
| 18 | 18px | 15.75px |
| 20 | 20px | 17.5px |
| 22 | 22px | 19.25px |
| 24 | 24px | 21px（大） |

### 3.4 字体字段（只影响终端）

```ts
// 保留值
"system-default"  →  xterm.options.fontFamily = "Consolas, Monaco, 'Courier New', monospace"

// 普通值
"JetBrains Mono"  →  xterm.options.fontFamily = "JetBrains Mono, Consolas, Monaco, 'Courier New', monospace"
```

xterm 的 fontFamily 接受 CSS 字体栈，**按顺序找第一个已安装的字体**，未装则自动回退到下一个——天然防御"列了不存在的字体"。

### 3.5 Tailwind rem 兼容性

- Tailwind 默认 `text-sm = 0.875rem` / `text-xs = 0.75rem` / `text-lg = 1.125rem`
- rem 跟着 `<html>` font-size 缩放
- **0 个组件 class 需要改**（前提：12-24 范围内布局不崩）

### 3.6 风险审计

实施前用 `grep -rE "(\bw-\[|h-\[|min-h-\[|p-\[|m-\[|gap-\[)" src/` 列出所有硬编码 px 尺寸，分类决定改/不改。预期 24px 下风险低（已审视现状），但**走查一遍再实施**。

---

## 4. 组件 / 数据流

### 4.1 改动文件清单

**Rust 端（4 文件）**：

| 文件 | 改动 |
|------|------|
| `src-tauri/Cargo.toml` | +`font-kit = "0.14"` |
| `src-tauri/src/fonts.rs`（新） | `list_mono_fonts() -> Vec<FontInfo>` 跨平台扫描 |
| `src-tauri/src/config_impl.rs` | `AppConfig` 加 `font_size: u32` + `font_family: String` + 兜底 + `Default` |
| `src-tauri/src/ipc/commands.rs` | +`cmd_list_fonts()` command；`cmd_load/save_config` 自动透传 |

**前端（10 文件）**：

| 文件 | 改动 |
|------|------|
| `src/utils/fonts.ts`（新） | 常量：`SYSTEM_DEFAULT_FAMILY`、`FONT_SIZE_RANGE`、`FONT_SIZE_LABELS`；纯函数：`clampFontSize`、`resolveFontFamily` |
| `src/stores/fontStore.ts`（新） | `fonts: FontInfo[]` + `loaded: boolean` + `loadFonts()` action |
| `src/stores/configStore.ts` | `AppConfigFE` interface 加 2 字段；`DEFAULT_CONFIG` 加默认值；`setFontSize(n)` / `setFontFamily(s)` action（带 clamp） |
| `src/hooks/useConfigSync.ts` | 启动调 `loadFonts`；新增 2 个 selector 订阅（`fontSize` / `fontFamily`）→ debounce 500ms 写盘 |
| `src/hooks/useFontSize.ts`（新） | 监听 `configStore.fontSize` → `document.documentElement.style.fontSize = '${n}px'` |
| `src/components/SerialToolbar.tsx` | +1 步进按钮组 + 字体 Combobox 容器 |
| `src/components/FontPicker.tsx`（新） | Combobox：input 过滤 + ul 列表 + 点击外部关闭 + 键盘交互（↑↓ 移动高亮 / Enter 选中 / Esc 关闭）。**列表第 1 项固定为"系统默认（当前）"** + 分隔线 + 字体列表 |
| `src/components/Terminal.tsx` | 初始化用 `useConfigStore` 读 `fontSize` + `fontFamily`；新增 useEffect 监听 store 变化 → `xterm.options.fontSize/fontFamily = ...` |
| `src/App.tsx` | +3 个 hotkey（`Ctrl++` / `Ctrl+-` / `Ctrl+0`） |
| `src/components/HotkeyHelp.tsx` | +3 行新快捷键说明。**新增"显示"分组**（字号放大 / 字号缩小 / 字号重置）—— 现有 `HotkeyHelp.tsx:54-66` 是单 table 无分组，需补分组渲染 |

### 4.2 启动数据流

```
Rust 启动 → config.json 加载 → AppConfig.fontSize=14, font_family="system-default"
  ↓
Webview 启动 → useConfigSync.loadFromBackend
  ↓
configStore.fontSize=14, font_family="system-default"
  ↓
useFontSize effect → documentElement.style.fontSize = '14px'   ← 全 UI rem 缩放
  ↓
Terminal useEffect 初始化 → xterm.options.fontSize=14, fontFamily='Consolas, ...'
  ↓
useConfigSync 调 loadFonts → fontStore.fonts = [...20-60 个等宽字体]
  ↓
SerialToolbar 渲染步进按钮 "14px" + FontPicker 渲染 Combobox
```

### 4.3 字号变化数据流（用户点 A+ 按钮）

```
SerialToolbar button click
  ↓
configStore.setFontSize(16)   ← 内置 clamp(12, 24)
  ↓
useConfigSync selector 订阅触发 → debounce 500ms → invoke("cmd_save_config")
  ↓ (立即)
useFontSize effect 监听到 → documentElement.style.fontSize = '16px'
  ↓
全 UI rem 重排
  ↓
（如果用户在终端）Terminal useEffect 监听到 → xterm.options.fontSize = 16
```

### 4.4 字体变化数据流（用户选 "JetBrains Mono"）

```
FontPicker li click
  ↓
configStore.setFontFamily("JetBrains Mono")
  ↓
useConfigSync → debounce 500ms → 写盘
  ↓ (立即)
Terminal useEffect → xterm.options.fontFamily = 'JetBrains Mono, Consolas, Monaco, ...'
  ↓
xterm 重绘
```

### 4.5 关键设计原则（沿用 CLAUDE.md 现有约定）

1. **store 写 → useEffect 同步 DOM**（不直接调 `document.*` 在组件里）
2. **持久化走 useConfigSync debounce**（与现有 `theme` 一致）
3. **xterm 更新用 `xterm.options.xxx = ...`**（已存在的 `theme` 切换模式，`Terminal.tsx:102-107`）
4. **不引入第三方 UI 库**（Combobox 手搓 ~50 行）
5. **Rust 单一来源**（`AppConfig` 字段加，前后端透传）
6. **乐观更新不阻断 UI**（CLAUDE.md 设计原则 #3）

### 4.6 字体列表缓存策略

- `fontStore.fonts` 启动时 invoke 一次缓存到内存
- 不持久化（OS 级数据，重启系统可能变）
- 性能：Windows 扫描 ~50-200ms（冷启动可接受）

---

## 5. 错误处理

### 5.1 9 个失败场景

| # | 场景 | 风险 | 处理 |
|---|------|------|------|
| 1 | 字号越界（>24 或 <12） | 用户 spam `Ctrl++` 撑爆布局 | `setFontSize` 内置 `clamp(12, 24)`；按钮 disabled 状态 |
| 2 | 快捷键 handler 越界 | 同上 | handler 调 `setFontSize`（clamp 集中） |
| 3 | 字体被卸载（极罕见） | 选中的字体消失 | xterm fontFamily 字符串自带 fallback 栈，自动兜底；log warn |
| 4 | `font-family: "system-default"` 保留值 | 与正常字体名混在一起 | store 写入 trim + 空串防御；Terminal useEffect 检测保留值 |
| 5 | `font-kit` 扫描失败（Linux 缺 fontconfig） | 启动卡 / 报错 | Rust 返回 `Vec::new()` + `log::warn!`；Combobox 显示"未找到等宽字体" |
| 6 | 持久化失败 / race | 写盘失败；500ms 内断电 | **复用现有模式**：乐观更新不阻断 UI；下次启动用旧值。**debounce 500ms 重置 timer**：`n` 次连续 `setFontSize` 只触发 1 次 `cmd_save_config`，**写的是最终值**；断电窗口 ≤ 500ms 内最多丢失 1 次中间态，**最终值会落盘** |
| 7 | 启动 race（Terminal mount 时 configStore 还没 load 完） | 闪屏 | store 初始值 `fontSize: 14, font_family: "system-default"` 兜底（**与原硬编码一致**） |
| 8 | 字体名注入 / XSS | 字体名不是用户输入 | Rust `String` 类型天然防御；前端 React 自动转义；CSS 字体值不执行 JS |
| 9 | Combobox UX 边界 | 长列表 / 无结果 / 空列表 | input 过滤 + max-h-60 滚动 + "无匹配"提示 + 空列表"未找到"提示 |

### 5.2 错误处理代码集中点

**不分散 try/catch**——把防御逻辑集中到 2 处：

```ts
// src/stores/configStore.ts
setFontSize: (n: number) => {
  const clamped = Math.max(12, Math.min(24, n));
  set({ fontSize: clamped });
},

setFontFamily: (name: string) => {
  set({ fontFamily: name.trim() || "system-default" });
},
```

```ts
// src/components/Terminal.tsx
useEffect(() => {
  if (!xtermRef.current) return;
  const fallback = SYSTEM_DEFAULT_FAMILY;
  const family = fontFamily === "system-default"
    ? fallback
    : `${fontFamily}, ${fallback}`;
  xtermRef.current.options.fontFamily = family;
}, [fontFamily]);
```

### 5.3 错误日志策略

- **Rust 端**：`log::warn!` / `log::error!`（CLAUDE.md 已规范）
- **前端**：**不弹 alert**，只 console.warn（保持工业工具"静默容错"风格）

### 5.4 不做的错误处理

- ❌ 字号"撤销"（连续步进无历史栈）
- ❌ 字体预览（hover 不显示效果）
- ❌ 字号缩放时的"防爆"clamp（12-24 已安全）

---

## 6. 测试策略

### 6.1 单元测试

**Rust（新增 ~4 个）**：

| 模块 | 测试 |
|------|------|
| `fonts.rs` | `list_mono_fonts_returns_valid_structure` |
| `fonts.rs` | `list_mono_fonts_handles_empty_gracefully`（CI Linux 无等宽时） |
| `config_impl.rs` | `appconfig_default_has_new_fields` |
| `config_impl.rs` | `appconfig_serde_backward_compat`（旧 config 无新字段 → 不报错） |

**前端（新增 ~14 个，vitest）**：

| 文件 | 测试 |
|------|------|
| `utils/fonts.ts` | `clampFontSize` 5 个 case（上下界 + 正常） |
| `utils/fonts.ts` | `resolveFontFamily` 2 个 case（保留值 / 普通值） |
| `stores/configStore.ts` | `setFontSize` 3 个 case（正常 / 上界 clamp / 下界 clamp） |
| `stores/configStore.ts` | `setFontFamily` 2 个 case（空串 → 保留值 / trim） |
| `components/FontPicker.tsx` | 渲染 + 过滤 + 选中 + Esc + 空态（5 个） |

### 6.2 组件 / 集成测试

**前端组件（vitest + @testing-library/react，新增 ~3 个）**：

| 文件 | 测试 |
|------|------|
| `components/Terminal.tsx` | 字号变化时 `xterm.options.fontSize` 被更新 |
| `components/Terminal.tsx` | 字体变化时 `xterm.options.fontFamily` 被更新 |
| `components/Terminal.tsx` | `fontFamily === "system-default"` 走 fallback 常量 |

**Rust 集成测试（`tests/` 新增 ~2 个）**：

| 测试 |
|------|
| `cmd_list_fonts_returns_valid_json` |
| `config_roundtrip_with_new_fields` |

### 6.3 手动验证清单（`tauri dev`，20 项）

**基础流（10）**：启动默认 14px / 点 A+ 步进 / 边界 disabled / 中间点重置 / Ctrl++/Ctrl+-/Ctrl+0 / 边界 clamp 生效

**字体流（6）**：Combobox 列表 / 输入过滤 / 选中切换 / "无匹配" / 系统默认 / Esc 关闭

**持久化（2）**：改字号保留 / 改字体保留

**布局（2）**：24px 无溢出 / 12px 可读

### 6.4 回归保护

- 主题切换不影响字号（独立维度）
- 终端数据收发（CH340 实测）正常
- 现有 169 测试全部通过
- Release build 成功

### 6.5 性能基线

- 启动时间增加 < 200ms（font-kit 扫描）
- 字号切换响应 < 16ms（一帧）
- Combobox 输入过滤无需 debounce（< 16ms 内联）

### 6.6 跨平台测试 + 不做的测试

**font-kit 跨平台**：

| 平台 | 系统依赖 | 行为 | 验证 |
|------|---------|------|------|
| Windows | 无（自带 DirectWrite） | 读注册表 `HKLM\..\Fonts`，~50-200ms | 本地手动 |
| macOS | 无（自带 CoreText） | CoreText enumerates | 本地手动 |
| Linux | **必须装 `libfontconfig1-dev`**（编译时硬依赖） | fontconfig enumerates | CI + 本地 |

**CI 配置（GitHub Actions）**：在 `.github/workflows/*.yml` 现有的 `apt install` 步骤加 `libfontconfig1-dev`（**v1.1.0 release blocker**——漏了 CI 编译失败）。

**手动清单补充**：实施期本地 Linux 开发机（Ubuntu / Debian）跑一次 `cargo build` + `cargo test` 确认 native build 链路通。

**不做的测试**：

- ❌ E2E 自动化（项目无 E2E 框架）
- ❌ 视觉回归截图（项目无 playwright/percy）

---

## 7. 实施步骤概要（详细计划由 writing-plans 落地）

1. **Rust 端**（先做）
   - 加 `font-kit` 依赖
   - 新 `fonts.rs` + `list_mono_fonts` + 单测
   - `AppConfig` 加 2 字段 + 兜底 + 单测
   - `cmd_list_fonts` command + 集成测试

2. **前端基础**（并行）
   - `utils/fonts.ts` 常量 + 纯函数 + 单测
   - `stores/fontStore.ts` + `stores/configStore.ts` 字段
   - `hooks/useFontSize.ts`
   - `Terminal.tsx` 接入 store + 组件测试

3. **前端 UI**
   - `FontPicker.tsx` Combobox + 组件测试
   - `SerialToolbar.tsx` 步进按钮 + 字体容器
   - `App.tsx` 3 个 hotkey
   - `HotkeyHelp.tsx` 3 行说明

4. **集成 & 验证**
   - `useConfigSync.ts` 集成
   - 12px / 24px 走查所有组件
   - 手动验证 20 项清单
   - 回归测试 169 个

---

## 8. 风险与权衡

| 风险 | 等级 | 缓解 |
|------|------|------|
| 硬编码 px 尺寸在 24px 下溢出 | 中 | 实施前 grep 审计 + 24px 走查 |
| `font-kit` 跨平台差异 | 低 | Linux 缺 fontconfig 时返回空（已设计降级） |
| `<html>` fontSize 缩放影响 modal / 浮层 | 低 | 现有 HotkeyHelp 是浮层，rem 化后跟着缩放，OK |
| Combobox 在长列表（60 项）下性能 | 低 | input 过滤即时（< 16ms），无需 debounce |
| 启动增加 200ms 用户感知 | 极低 | 比 Vite 705ms + Cargo 2.13s 小一个数量级 |
| xterm fontFamily 切换导致光标重置 | 低 | xterm 内部处理，不破坏数据流 |

---

## 9. 替代方案（已否决）

### 9.1 方案 1（已否决）：CSS 变量 + 4 档

- 4 档固定：12/14/16/18
- 改 `<html>` 的 class（`html.font-sm/md/lg/xl`）
- 用户已明确要求支持更多档位 → 否定

### 9.2 不做 UI 字体切换（已确认）

- 用户要求"只给终端用"（即显示的收发数据）
- 改 UI 字体可能让布局崩（中文/英文宽度差异）→ 不做

### 9.3 不用 font-kit（已否定）

- Web `document.fonts` API **不能列出系统字体**（只能查已加载的 @font-face）
- 必须 Rust 端扫

### 9.4 引入 Headless UI / Radix Combobox（已否定）

- 给 Tauri 项目加无谓的运行时依赖
- 自己手搓 ~50 行就够

---

## 10. 文档更新计划

- ✅ `docs/superpowers/specs/2026-06-12-font-size-toggle-design.md`（本文件）
- ⏳ `docs/superpowers/plans/2026-06-12-font-size-toggle-impl.md`（writing-plans 落地）
- ⏳ `README.md` 同步新功能（update-readme skill）
- ⏳ `CLAUDE.md` 同步 v1.1.0 增量（update-claudemd skill）
- ⏳ `docs/releases/v1.1.0.md` 发布 notes

---

## 11. 元信息

- **预估代码量**：~400 行（前端 ~300 + Rust ~100）
- **预估工时**：半天到 1 天
- **预估测试**：~20 个自动化测试 + 20 项手动
- **依赖项**：
  - Rust：`font-kit = { version = "0.14", default-features = true }`（新增 1 个，默认 feature 全平台 OK）
  - Linux 系统包：`libfontconfig1-dev`（**编译时依赖**，CI / Linux 开发机必须装）
- **版本号影响**：v1.0.2 → v1.1.0（新增功能，minor bump）
- **向后兼容**：旧 config.json 无新字段 → serde `#[serde(default)]` 兜底，不破坏用户数据
