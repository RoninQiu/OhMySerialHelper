# OhMySerial

> 面向工业控制的高性能串口调试助手 — Rust + Tauri 2.x + React

[![Version](https://img.shields.io/badge/version-1.2.0-blue.svg)](https://github.com/RoninQiu/OhMySerialHelper/releases)
[![Platform](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://www.microsoft.com/windows)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![AI](https://img.shields.io/badge/built%20with-AI%20Assisted-purple.svg)](#-关于本项目)

OhMySerial 是一款面向工业控制和嵌入式开发的现代化串口调试助手，旨在替代传统 SSCOM 等老旧工具。通过 **Rust 异步内核** 与 **WebGL 加速渲染** 的结合，解决传统工具在大数据量下卡顿、不支持无损 HEX 切换、定时器不精准等痛点。

> 🤖 **本项目使用 AI 辅助开发** — 核心代码、文档、测试和 CI 流程由 AI 编码助手（Claude、Kimi Code 等）协助完成。架构设计、需求决策、测试由人类开发者主导，AI 负责编码加速、文档同步和重构建议。

<!-- TODO: 截图 - 替换为实际应用截图 -->

## ✨ 特性亮点

- 🚀 **零拷贝高性能内核** — Rust 异步读取线程 + 64KB RingBuffer + Channel 零拷贝 IPC，`write_4KB` 吞吐 92 GiB/s
- 🖥️ **WebGL 加速终端** — Xterm.js 5.5 渲染，每行带 `[HH:MM:SS.mmm] ←/→` 时间戳与收发方向（RX 蓝字 / TX 绿字）
- 📤 **完整发送能力** — 文本 / HEX 发送、预设命令 CRUD、轮询队列、单 payload 周期发送，TX 自动回显
- 🎬 **收发录制** — 一键把 RX + TX + 系统消息写入本地 `.txt`；断线重连不切文件，写注释行标记数据缺口
- 🚨 **断线检测 + 自动重连** — 分级错误处理 + 指数退避 1/2/4/8/15s（最多 5 次，可取消）
- 🔌 **USB 芯片自动识别** — 按 VID/PID 识别 CH340 / FTDI / CP210x / PL2303 等，显示形如 `COM3 (CH340 · wch.cn)`
- ⚙️ **工程化体验** — 深 / 浅 / 跟随系统三主题、字号与等宽字体可调、配置原子写持久化、抽屉式日志面板；快捷键 `Ctrl+L` 清屏 / `Ctrl+K` 聚焦发送 / `Ctrl+T` 切主题 / `F1` 帮助 / `F2` 日志

## 📦 快速开始

### 环境要求

| 依赖 | 版本 |
|------|------|
| Node.js | 20+ |
| Rust | stable 1.77+ |
| 系统 | Windows 10/11（当前仅支持 Windows） |
| WebView2 Runtime | Win11 预装；Win10 需[手动安装](https://developer.microsoft.com/microsoft-edge/webview2/) |

### 安装与运行

```bash
git clone https://github.com/RoninQiu/OhMySerialHelper.git
cd OhMySerialHelper
npm install
npm run tauri dev          # 启动 Vite + Tauri，支持热重载
```

不想自己构建？到 [Releases](https://github.com/RoninQiu/OhMySerialHelper/releases) 下载 `OhMySerial_1.2.0_x64-setup.exe`，双击安装即可使用，无需任何配置。

## 🔨 构建发布

```bash
npm run tauri build
# 产物：target/release/bundle/nsis/OhMySerial_<version>_x64-setup.exe（约 12MB）
```

## 🛠 技术栈

| 层级 | 技术 |
|------|------|
| 后端框架 | Tauri 2.x |
| 后端语言 | Rust (tokio + serialport) |
| 后端日志 | fern + chrono（7 天滚动） |
| 前端框架 | React 18 + TypeScript |
| 状态管理 | Zustand + persist 中间件 |
| 终端渲染 | Xterm.js 5.5 (WebGL) |
| 样式 | Tailwind CSS 3 (darkMode: class) |
| 构建工具 | Vite 5 |
| 测试 | Vitest + Cargo test + criterion bench |

## 📁 项目结构

```
OhMySerialHelper/
├── core/                     # 核心业务逻辑（UI-agnostic Rust lib）
│   └── src/                  #   serial/ · sender/ · recorder/ · backend · config · log_init
├── src-tauri/                # Tauri 2.x 主机（薄壳）
│   ├── src/                  #   ipc/commands.rs（27 个 IPC）· lib.rs（事件转发）
│   └── tests/                #   Rust 集成测试（29 个，需 CH340 硬件）
├── src/                      # React 前端
│   ├── components/           #   Terminal · SerialToolbar · SendPanel · PresetPanel
│   │                         #   StatusBar · LogPanel · SettingsPanel · FontPicker
│   ├── stores/               #   Zustand（serial / buffer / preset / ui / config / log / recorder / font）
│   ├── hooks/                #   useHotkeys · useThemeClasses · useRafValue · useConfigSync · useLogPolling
│   └── utils/                #   hex · encoding · format · terminalFormat · logParser · fonts
├── tests/frontend/           # 前端单元测试（190 个）
└── docs/                     # 设计文档 · 性能基准报告 · 发布说明
```

## 🧪 测试

```bash
npm test                                    # 前端单元测试（190 个）
cargo test -p oh-my-serial-core             # 核心层单元测试（58 个）
cd src-tauri && cargo bench --features bench # 性能基准（criterion）
```

集成测试需要真实 CH340 硬件（TX-RX 短接），CI 上用 com0com 虚拟串口对替代：

```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM5"
cd src-tauri
cargo test --test env_check --test scenario_basic_echo --test scenario_large_transfer --test scenario_disconnect --test scenario_ipc_e2e --test scenario_polling
```

性能基准结果见 [docs/bench-v0.6.0.md](docs/bench-v0.6.0.md)。

## 🗺 路线图

- [x] **v0.1.0 – v0.4.0** — 基础框架、数据接收打通、发送闭环、主题 / 快捷键 / 文件日志 / 性能基准
- [x] **v0.5.0** — 配置持久化（`config.json` 原子写）+ 自动重连（指数退避）
- [x] **v0.6.0** — Channel 零拷贝 + RingBuffer chunked memcpy（≈625×）+ 时间戳 / 收发方向
- [x] **v1.0.0 – v1.0.2** — 日志面板（`F2`）+ VID/PID 精准识别 + UI 版本号动态同步
- [x] **v1.1.0 – v1.1.2** — 字号 / 字体可调 + 布局防御硬化 + 预设命令简化（所见即所发）
- [x] **v1.2.0** — 收发录制（重连不切文件）+ Settings Modal
- [ ] **v1.3.0** — 核心层抽离为 `oh-my-serial-core`（cargo 工作区，供多宿主复用）

## 🤝 贡献

欢迎 Issue 和 PR！

1. Fork 仓库，创建 feature 分支（`git checkout -b feature/xxx`）
2. 提交前跑测试（见 [🧪 测试](#-测试)），确保全绿
3. 提交（`git commit -m "feat: xxx"`），Push 分支并创建 PR

## 📚 设计文档

- [架构设计](docs/plans/2026-05-29-OhMySerial-design.md) — 数据流、背压策略、IPC 设计
- [设计决策记录](docs/design-decisions.md) — 34 条关键决策与教训
- [性能基准报告 v0.6.0](docs/bench-v0.6.0.md) — chunked memcpy + Channel 零拷贝
- [各版本发布说明](docs/releases/)

## 📄 许可证

[MIT](LICENSE) © RoninQiu
