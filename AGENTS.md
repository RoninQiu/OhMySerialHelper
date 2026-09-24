# OhMySerial — AGENTS.md

> 本项目面向 AI 编码助手，提供项目架构、技术栈、开发约定和测试策略的完整说明。

## 项目概览

OhMySerial 是一款面向工业控制和嵌入式开发的现代化串口调试助手，基于 **Rust 异步内核 + WebGL 加速渲染**，替代传统 SSCOM 等老旧工具。

- **版本**: 1.2.0
- **许可证**: MIT
- **平台**: Windows 10/11 (仅 Windows)
- **仓库**: https://github.com/RoninQiu/OhMySerialHelper

## 技术栈

| 层级     | 技术                                       |
| -------- | ------------------------------------------ |
| 后端框架 | Tauri 2.x                                  |
| 后端语言 | Rust (tokio + serialport)                  |
| 后端日志 | fern + chrono（7 天滚动文件日志）          |
| 前端框架 | React 18 + TypeScript                      |
| 状态管理 | Zustand (subscribeWithSelector middleware) |
| 终端渲染 | Xterm.js 5.5 (WebGL)                       |
| 样式     | Tailwind CSS 3 (darkMode: class)           |
| 构建工具 | Vite 5                                     |
| 前端测试 | Vitest (jsdom)                             |
| 后端测试 | Cargo test + criterion bench               |
| 构建产物 | NSIS 安装包 (~12MB)                        |

## 项目结构

```
OhMySerialHelper/
├── core/                          # 核心 Rust 业务逻辑 (UI-agnostic)
│   └── src/
│       ├── serial/port.rs         # 串口枚举 + USB VID/PID 识别
│       ├── serial/ring_buffer.rs  # 64KB 环形缓冲区 (chunked memcpy)
│       ├── sender/                # 发送队列 (SendQueue + PreciseSender)
│       ├── recorder/              # 录制器 (v1.2.0)
│       ├── backend.rs             # Backend 句柄 (Tauri/egui 共用)
│       ├── config.rs              # AppConfig 持久化 (JSON 原子写)
│       ├── error.rs               # SerialError 类型
│       ├── log_init.rs            # 日志系统初始化 + 日志行读取
│       ├── fonts.rs               # 系统字体枚举 (font-kit)
│       └── lib.rs                 # 公开 API 重新导出
├── src-tauri/                     # Tauri 2.x 桌面主机
│   ├── src/
│   │   ├── ipc/commands.rs        # 27 个 Tauri 命令薄壳
│   │   ├── lib.rs                 # 应用入口 + 事件转发
│   │   └── main.rs                # 程序入口
│   ├── benches/                   # criterion 性能基准 (4 个)
│   ├── tests/                     # 集成测试 (29 个, 需硬件)
│   ├── capabilities/              # Tauri 2.x 权限配置
│   └── tauri.conf.json            # Tauri 配置
├── egui-app/                      # 实验性 egui UI (A/B 对比)
│   └── src/
│       ├── main.rs                # egui 入口
│       ├── app.rs                 # EguiApp 主循环
│       ├── backend_bridge.rs      # Backend → egui 桥接
│       ├── state.rs               # 跨线程共享状态
│       ├── terminal.rs            # 终端缓冲区
│       └── ui/                    # egui UI 组件
├── src/                           # React 前端
│   ├── components/                # UI 组件
│   │   ├── Terminal.tsx           # Xterm.js 渲染
│   │   ├── SerialToolbar.tsx      # 串口工具栏
│   │   ├── SendPanel.tsx          # 发送面板
│   │   ├── PresetPanel.tsx        # 预设命令 CRUD
│   │   ├── StatusBar.tsx          # 状态栏 (rAF 节流 15Hz)
│   │   ├── LogPanel.tsx           # 日志面板 (抽屉式)
│   │   ├── HotkeyHelp.tsx         # 快捷键帮助浮层
│   │   ├── SettingsPanel.tsx      # 设置弹窗 (v1.2.0)
│   │   └── FontPicker.tsx         # 字体选择器
│   ├── stores/                    # Zustand 状态管理
│   │   ├── serialStore.ts         # 串口连接 + sendData + Channel
│   │   ├── bufferStore.ts         # 收发字节统计 (60Hz 累积)
│   │   ├── presetStore.ts         # 预设命令 (localStorage)
│   │   ├── uiStore.ts             # 主题 (持久化 + matchMedia)
│   │   ├── configStore.ts         # Rust 配置镜像 + auto-save
│   │   ├── logStore.ts            # 日志缓存 + 过滤
│   │   ├── recorderStore.ts       # 录制状态 (v1.2.0)
│   │   └── fontStore.ts           # 字体列表缓存
│   ├── hooks/                     # 自定义 Hook
│   │   ├── useHotkeys.ts          # 全局快捷键
│   │   ├── useThemeClasses.ts     # 主题 class 助手
│   │   ├── useRafValue.ts         # rAF 节流
│   │   ├── useConfigSync.ts       # 多 store → configStore 同步
│   │   └── useLogPolling.ts       # 2s 轮询日志
│   └── utils/                     # 工具函数
│       ├── hex.ts                 # HEX 解析 + CRC16
│       ├── encoding.ts            # GBK/UTF-8 编解码
│       ├── format.ts              # bytesToHuman
│       ├── terminalFormat.ts      # 终端格式化 (v1.2.0)
│       ├── fonts.ts               # 字号范围常量
│       ├── version.ts             # 版本号工具
│       └── logParser.ts           # 日志行解析
├── tests/frontend/                # 前端单元测试 (Vitest, 190 个)
│   ├── mocks/tauri.ts             # Tauri API mock
│   ├── hex.test.ts
│   ├── serialStore.integration.test.ts
│   ├── bufferStore.test.ts
│   ├── uiStore.test.ts
│   ├── useHotkeys.test.ts
│   ├── terminal.test.ts
│   ├── terminalFormat.test.ts
│   ├── configStore.test.ts
│   ├── recorderStore.test.ts
│   ├── logStore.test.ts
│   ├── logParser.test.ts
│   ├── presetStore.test.ts
│   ├── fonts.test.ts
│   ├── fontListGuard.test.ts
│   ├── fontSizeUiGuard.test.ts
│   ├── format.test.ts
│   ├── useRafValue.test.ts
│   ├── useThemeClasses.test.ts
│   ├── useConfigSync.test.ts
│   ├── configStoreFont.test.ts
│   └── components/               # 组件测试
│       ├── SettingsPanel.test.tsx
│       ├── PresetPanel.test.tsx
│       ├── FontPicker.test.tsx
│       └── TerminalFont.test.ts
├── docs/                          # 设计文档 + 性能基准报告
│   ├── bench-v0.4.0.md            # byte-loop 基线
│   ├── bench-v0.6.0.md            # chunked memcpy + Channel 零拷贝
│   ├── design-decisions.md        # 架构决策记录
│   ├── plans/                     # 实施计划
│   └── releases/                  # 版本发布说明
├── scripts/release.sh             # 版本发布自动化脚本 (本地, 不入 Git)
├── .github/workflows/test.yml     # CI 配置
├── Cargo.toml                     # 工作区根配置
└── package.json                   # 前端依赖 + 脚本
```

## 架构设计

### 三层架构

```
┌─────────────────────────────────────────────────────────────────┐
│  React 前端 (src/)                                              │
│  Zustand stores + Xterm.js + Tailwind CSS                      │
│  通过 Tauri IPC invoke 调用后端                                 │
│  事件监听: port-disconnected, reconnect-status, send-*-error    │
└──────────────────────────────┬──────────────────────────────────┘
                               │ Channel<Vec<u8>> (零拷贝)
                               │ Tauri IPC invoke (27 个命令)
┌──────────────────────────────┴──────────────────────────────────┐
│  src-tauri/ (Tauri 2.x 主机)                                    │
│  ipc/commands.rs: 27 个薄壳命令, 1-3 行 delegate 到 core       │
│  lib.rs: 事件转发 (BackendEvent → app.emit)                    │
└──────────────────────────────┬──────────────────────────────────┘
                               │ 共享 core 库
┌──────────────────────────────┴──────────────────────────────────┐
│  core/ (UI-agnostic Rust 业务逻辑)                              │
│  Backend: serialport 驱动 + RingBuffer + SendQueue + Recorder   │
│  tokio::sync::mpsc 数据通道 (替代 tauri::Channel)               │
│  tokio::sync::broadcast 事件通道 (替代 app.emit)                │
└─────────────────────────────────────────────────────────────────┘
```

### 数据流 (接收路径)

1. `serialport` 驱动在 `serial-reader` 线程中循环读取 256B 块
2. 数据写入 `64KB RingBuffer` (chunked memcpy, ~92 GiB/s)
3. 触发条件 (满 4KB 或 16ms 超时) → `drain_all()` 读取积压数据
4. 通过 `mpsc::Sender<Vec<u8>>` 推送到桥接层
5. Tauri 桥接层: `mpsc::Receiver` → `Channel<Vec<u8>>` (零拷贝)
6. Xterm.js 接收并渲染 (WebGL 加速)

### 断线检测 + 自动重连

- `NotConnected` / `BrokenPipe` 直接触发断线
- 其他 IO 错误累积 3 次后判定断线 (`DISCONNECT_ERROR_THRESHOLD`)
- `TimedOut` 不视为断线, 计数器清零
- 重连退避: 1s → 2s → 4s → 8s → 15s (最多 5 次)
- 重连期间不中断录制 (v1.2.0)

### 发送路径

- **SendPanel** (直接发送): invoke `cmd_write_data` → 立即写入串口
- **SendQueue** (轮询发送): 启动 `send-poller` 线程, 按 interval 逐个发送
- **PreciseSender** (定时发送): 启动 `send-precise` 线程, 周期发送固定 payload

## 开发命令

```bash
# 安装依赖
npm install

# 开发模式 (Vite 热重载 + Tauri)
npm run tauri dev

# 仅启动 Vite 前端开发服务器
npm run dev

# 前端单元测试 (190 个)
npm test

# 核心层单元测试 (58 个)
cargo test -p oh-my-serial-core

# Rust 集成测试 (29 个, 需 CH340 硬件, TX-RX 短接)
$env:OH_MY_SERIAL_TEST_PORT = "COM5"
cd src-tauri && cargo test --test env_check --test scenario_basic_echo --test scenario_large_transfer --test scenario_disconnect --test scenario_ipc_e2e --test scenario_polling

# 性能基准测试
cd src-tauri && cargo bench --features bench

# 生产构建
npm run tauri build
# 产物: target/release/bundle/nsis/OhMySerial_1.2.0_x64-setup.exe
# （v1.3.0 转 cargo 工作区后构建产物在仓库根 target/，不再是 src-tauri/target/）

# Rust 检查
cd src-tauri && cargo clippy -- -D warnings
```

## 核心约定

### 版本号同步

版本号定义在 **3 个文件** 中, 发布时需同时更新:

1. `src-tauri/Cargo.toml` — Rust 工作区版本
2. `src-tauri/tauri.conf.json` — Tauri 应用版本
3. `package.json` — 前端版本

使用 `scripts/release.sh` 自动化发布流程。

### 前端约定

- **TypeScript strict mode**: `noUnusedLocals`, `noUnusedParameters`, `strict` 均启用
- **路径别名**: `@/` 映射到 `src/` (Vite config)
- **状态管理**: 使用 Zustand, 优先用 `subscribeWithSelector` middleware 做按需订阅
- **主题系统**: 通过 CSS 变量 (`--bg-primary`, `--text-primary` 等) + `html.light` / `html.dark` class 切换
- **快捷键**: 在 `App.tsx` 中集中注册, 使用 `useHotkeys` hook
- **字体**: 字号只影响终端 (xterm.options.fontSize), 不动 `<html>` 字号, 避免撑破布局
- **录制前端**: `recorderStore` 管理录制状态, `terminalFormat` 导出纯函数用于格式化行

### Rust 约定

- **错误处理**: 使用 `thiserror` 派生 `SerialError` 枚举, 所有后端方法返回 `Result<_, SerialError>`
- **日志**: 使用 `log` crate (info/warn/error), 运行时输出到文件 + stderr
- **配置持久化**: 原子写 (tmp + rename), 启动时自动加载, 设置变更 debounce 500ms 写盘
- **基准测试**: 位于 `src-tauri/benches/`, 使用 `criterion` crate, 需 `--features bench`
- **core 库**: 不依赖任何 UI 框架 (Tauri/egui), 数据走 `mpsc::Sender`, 事件走 `broadcast::Sender`

### 集成测试约定

- RX-TX 物理短接 (CH340 自发自收)
- 环境变量 `OH_MY_SERIAL_TEST_PORT` 指定 COM 号
- 全局硬件互斥锁防止并行测试冲突
- CI 中使用 com0com 虚拟串口对

## 测试策略

### 前端测试 (190 个)

- 框架: Vitest, jsdom 环境
- 模式: 纯函数测试 + Zustand store 集成测试
- 组件测试: 使用 Vitest 渲染, 验证核心交互逻辑
- Mock: `tests/frontend/mocks/tauri.ts` 提供 `mockInvoke` + `eventListeners`
- Tauri API mock: `@tauri-apps/api/core` 的 `invoke` 和 `Channel` 通过 `vi.mock` 注入
- 运行: `npm test`

### Rust 单元测试 (58 个, 位于 core/)

- 位置: `core/src/**` 与源代码同文件 (`#[cfg(test)] mod tests`)
- 覆盖: ring_buffer, send_queue, log_init (7 天清理 + parse_line), reconnect (退避序列), recorder (8 个), serial/port (VID/PID), backend
- 运行: `cargo test -p oh-my-serial-core`
- 注意: `cargo test --lib` **不跑 doctest**, 要跑整套 (含 doctest) 用上面的命令

### Rust 集成测试 (29 个)

- 位置: `src-tauri/tests/`
- 需要 CH340 硬件 (TX-RX 短接)
- 覆盖场景: 环境检测, 基础 echo, 大数据量 (8KB), 断线检测 + 重连, IPC E2E, SendQueue 轮询
- 使用 `common/mod.rs` 中的 `hardware_available()` 守卫

### 性能基准 (4 个)

- ring_buffer_write (write_4KB: ~92 GiB/s)
- ring_buffer_cycle (write_256B_then_read: ~6 GiB/s)
- drain_all (drain_4KB: ~26 GiB/s)
- send_queue (add_256_to_queue: ~3 µs)

## 构建与发布

### 构建配置

- Rust release 优化: `lto = true`, `opt-level = "z"`, `strip = true`
- 后端编译: 通过 `tauri-build` 自动调用
- 前端构建: `tsc && vite build`, 产物输出到 `dist/`
- 前端产物在 Tauri 构建时嵌入到 `target/release/` (v1.3.0 转工作区后产物在仓库根 `target/`)

### 发布流程

由 `scripts/release.sh` 自动化完成:

1. 预检 (git 干净 / 在 main / gh 已登录)
2. 同步 3 处 version 字段 + README 机械引用
3. 重算 Cargo.lock
4. 跑测试
5. commit + push
6. 打 tag + push tag
7. 准备 release notes
8. gh release create + 上传 NSIS installer

### CI (GitHub Actions)

- 触发条件: push/PR 到 main
- 步骤: Node.js 20 + Rust stable → sccache → 前端测试 → 核心层单测 (`cargo test -p oh-my-serial-core`) → clippy → release build
- 集成测试: CI 通过 com0com 虚拟串口对实现, 无需硬件
- 注意: `cd src-tauri && cargo clippy -- -D warnings` 会**连带 lint `core/`** (cargo clippy 默认也检查本地 path 依赖); 该步骤带 `continue-on-error: true`, 即 lint 失败不会让 CI 变红

## 安全注意事项

- **串口权限**: 操作串口需要系统授权 (管理员权限或组策略)
- **日志文件**: 7 天滚动, 位于 `<exe>/logs/`, 不包含敏感凭据
- **配置持久化**: 原子写 (tmp + rename), 防止崩溃导致文件损坏
- **录制文件**: 用户指定路径, 纯文本格式, 不自动上传
- **前端 mock**: 测试中 mock Tauri API, 避免执行真实 IPC

## 设计文档

关键设计文档位于 `docs/` 目录:

- `docs/plans/2026-05-29-OhMySerial-design.md` — 架构设计、数据流、背压策略、IPC 设计
- `docs/plans/2026-05-29-OhMySerial-implementation.md` — 实施计划
- `docs/bench-v0.6.0.md` — 性能基准报告 (chunked memcpy + Channel 零拷贝)
- `docs/design-decisions.md` — 架构决策记录

## AI 协作者本地配置（不入 Git）

以下文件只存在于本地（`.gitignore` 已排除整个 `.agents/`）：

| 位置 | 内容 |
|------|------|
| `.agents/memory.md` | 会话记忆：必踩的坑、工作流约定、当前状态（由 `update-memory` skill 维护） |
| `.agents/rules/` | 细粒度规则：`ui-contrast` / `rust-safety` / `tauri-capabilities` |
| `.agents/skills/` | AI 工作流：`release`（发版）/ `update-readme`（同步 README）/ `update-memory`（同步记忆） |
| `.agents/settings.local.json` | 工具权限白名单（含本机路径） |
