# OhMySerial

> 面向工业控制的高性能串口调试助手 — Rust + Tauri 2.x + React

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/RoninQiu/OhMySerialHelper/releases)
[![Platform](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://www.microsoft.com/windows)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

OhMySerial 是一款面向工业控制和嵌入式开发的现代化串口调试助手，旨在替代传统 SSCOM 等老旧工具。通过 **Rust 异步内核** 与 **WebGL 加速渲染** 的结合，解决传统工具在大数据量下卡顿、不支持无损 HEX 切换、定时器不精准等痛点。

<!-- TODO: 截图 - 替换为实际应用截图 -->

## ✨ 特性亮点

- 🚀 **高性能异步内核** — Rust 后台读取线程 + 64KB 环形缓冲 + 4KB/16ms 批量 IPC，921600 高波特率不卡顿
- 🖥️ **WebGL 加速终端** — Xterm.js 5.5 渲染，60 FPS 流畅刷新
- 🔄 **智能背压策略** — 水位 50% / 75% / 90% 三档自适应，前端不积压
- 🔌 **常见芯片自动识别** — CH340 / FTDI / CP210x / PL2303 一键识别
- 📦 **精简单文件发布** — NSIS 打包，绿色免安装
- 🧪 **真实硬件集成测试** — 19 个测试覆盖基础收发、大数据量、断线重连

## 📦 快速开始

### 环境要求

- **Node.js** 20+
- **Rust** stable (1.75+)
- **Windows 10/11** （项目当前仅支持 Windows）
- WebView2 Runtime（Win11 预装，Win10 需手动安装）

### 安装与运行

```bash
# 克隆仓库
git clone https://github.com/RoninQiu/OhMySerialHelper.git
cd OhMySerialHelper

# 安装前端依赖
npm install

# 开发模式（启动 Vite + Tauri，自动热重载）
npm run tauri dev
```

启动后窗口会显示串口列表、波特率、缓冲区大小等配置，连接串口即可收发数据。

## 🔨 构建发布

```bash
# 生产构建
npm run tauri build

# 产物位置
src-tauri/target/release/bundle/nsis/OhMySerial_0.1.0_x64-setup.exe
```

构建配置已优化体积（`lto = true`, `opt-level = "z"`），单安装包约 12MB。

## 🛠 技术栈

| 层级 | 技术 |
|------|------|
| 后端框架 | Tauri 2.x |
| 后端语言 | Rust (tokio + serialport) |
| 前端框架 | React 18 + TypeScript |
| 状态管理 | Zustand |
| 终端渲染 | Xterm.js 5.5 (WebGL) |
| 样式 | Tailwind CSS 3 |
| 构建工具 | Vite 5 |
| 测试 | Vitest + Cargo test |

## 📁 项目结构

```
OhMySerialHelper/
├── src/                          # React 前端
│   ├── components/               # UI 组件 (Terminal, SerialToolbar)
│   ├── stores/                   # Zustand 状态管理
│   └── utils/                    # HEX/编码工具
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── serial/               # 串口驱动 + 环形缓冲
│   │   ├── ipc/                  # Tauri IPC 命令
│   │   └── sender/               # 发送队列 + 高精度定时器
│   ├── capabilities/             # Tauri 2.x 权限配置
│   └── tauri.conf.json
├── tests/frontend/               # 前端集成测试
├── docs/plans/                   # 设计与实施计划
├── CLAUDE.md                     # AI 助手指引
└── README.md                     # 本文件
```

## 🧪 测试

### 前端测试

```bash
npm test
```

### 集成测试（需真实 CH340 硬件）

```powershell
# 接线：CH340 的 TX 与 RX 用杜邦线短接（GND 也接上）

# 设备管理器查看 COM 号，假设为 COM5
$env:OH_MY_SERIAL_TEST_PORT = "COM5"

cd src-tauri
cargo test --test env_check --test scenario_basic_echo --test scenario_large_transfer --test scenario_disconnect --test scenario_ipc_e2e
```

**CI 模式**：GitHub Actions 自动安装 com0com 虚拟串口对，无需硬件。

## 🗺 路线图

- [x] **v0.1.0** — 基础框架、IPC、环形缓冲区、Xterm.js 组件
- [x] **v0.2.0** — 数据接收打通 + 19 个集成测试
- [ ] **v0.3.0** — UI/UX 优化（预设命令面板、定时发送、主题切换）
- [ ] **v0.4.0** — 性能基准测试（921600 压力 + 内存监控）
- [ ] **v1.0.0** — 自动重连 / 断线检测、本地配置持久化、日志记录
- [ ] **v1.1.0** — 跨平台支持（macOS / Linux）

## 🤝 贡献

欢迎 Issue 和 PR！

1. Fork 仓库
2. 创建 feature 分支（`git checkout -b feature/xxx`）
3. 提交前跑测试：
   ```bash
   npm test
   # 集成测试（需硬件）
   OH_MY_SERIAL_TEST_PORT=COM5 cargo test --test ...
   ```
4. 提交（`git commit -m "feat: xxx"`）
5. Push 分支并创建 PR

详细的开发上下文请参考 [CLAUDE.md](CLAUDE.md)，包含架构、当前状态、已知问题。

## 📚 设计文档

- [架构设计](docs/plans/2026-05-29-OhMySerial-design.md) — 数据流、背压策略、IPC 设计
- [实施计划](docs/plans/2026-05-29-OhMySerial-implementation.md) — 历史 Task 1-14 详细步骤
- [Task 12 集成测试计划](docs/superpowers/plans/2026-06-01-task12-integration-tests.md)

## 📄 许可证

[MIT](LICENSE) © RoninQiu
