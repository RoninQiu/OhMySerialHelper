# OhMySerial

> 面向工业控制的高性能串口调试助手 — Rust + Tauri 2.x + React

[![Version](https://img.shields.io/badge/version-1.0.2-blue.svg)](https://github.com/RoninQiu/OhMySerialHelper/releases)
[![Platform](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://www.microsoft.com/windows)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![AI](https://img.shields.io/badge/built%20with-AI%20Assisted-purple.svg)](#-关于本项目)

OhMySerial 是一款面向工业控制和嵌入式开发的现代化串口调试助手，旨在替代传统 SSCOM 等老旧工具。通过 **Rust 异步内核** 与 **WebGL 加速渲染** 的结合，解决传统工具在大数据量下卡顿、不支持无损 HEX 切换、定时器不精准等痛点。

> 🤖 **本项目使用 AI 辅助开发** — 核心代码、文档、测试和 CI 流程由 Claude（Anthropic）协助完成。架构设计、需求决策、测试由人类开发者主导，AI 负责编码加速、文档同步和重构建议。

<!-- TODO: 截图 - 替换为实际应用截图 -->

## ✨ 特性亮点

- 🚀 **零拷贝高性能内核** — Rust 后台读取线程 + 64KB RingBuffer (chunked memcpy ~92 GiB/s) + Tauri `Channel<Vec<u8>>` 跨进程零拷贝
- 🖥️ **WebGL 加速终端** — Xterm.js 5.5 渲染，每行带 `[HH:MM:SS.mmm] ←/→` 时间戳 + 收/发方向（RX 蓝字 / TX 绿字）
- 📤 **真正可发数据** — SendPanel（文本/HEX + Enter 发送 + onSent 回显）+ PresetPanel（CRUD + localStorage）+ SendQueue 轮询 + 单 payload 周期发送
- 🚨 **断线 + 自动重连** — 分级错误处理 + 指数退避 1/2/4/8/15s（最多 5 次，可取消）；CH340 拔出后 2s 内告警 + 自动恢复
- 💾 **本地配置持久化** — Rust serde + 原子写 (tmp + rename)，启动自动加载 + 设置变更 debounce 500ms 写盘
- 📋 **前端日志面板** — 抽屉式 LogPanel（F2 切换），2s 自动轮询，按级别/关键字过滤，一键打开日志目录
- 📊 **状态栏实时显示** — 连接状态 + TX/RX 字节（rAF 节流到 15Hz，源仍 60Hz 累积）+ 溢出计数 + 日志目录入口
- 🎨 **三主题切换** — 深色 / 浅色 / 跟随系统；WCAG AA 浅色模式可读性已验证
- ⌨️ **全局快捷键** — `Ctrl+L` 清屏、`Ctrl+T` 切主题、`Ctrl+K` 聚焦发送框、`F1`/`?` 帮助浮层、`F2` 切日志面板
- 📝 **文件日志** — fern 滚动日志，保留 7 天，写入 `<exe>/logs/oh-my-serial-YYYY-MM-DD.log`
- 🔌 **USB 芯片自动识别** — 用 USB VID/PID 精准识别 CH340 / FTDI / CP210x / PL2303 / MCP / XR21V / TUSB3410；识别不到时去掉无意义的 `(Unknown)` 后缀，有 manufacturer 时显示 `COM3 (CH340 · wch.cn)`
- 🧪 **169 测试** — 40 Rust 单测 + 12 集成 + 117 前端

## ⌨️ 快捷键速查

| 组合 | 功能 |
|------|------|
| `Ctrl+L` | 清空终端 |
| `Ctrl+T` | 循环切换主题（暗 → 亮 → 跟随系统） |
| `Ctrl+K` | 聚焦到发送输入框 |
| `F1` / `?` | 打开快捷键帮助浮层（`Esc` 关闭） |
| `F2` | 切换日志面板 |
| `Enter`（在发送框） | 发送当前输入 |
| `Ctrl+Enter`（在发送框） | 在输入中插入换行 |

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
src-tauri/target/release/bundle/nsis/OhMySerial_1.0.2_x64-setup.exe
```

构建配置已优化体积（`lto = true`, `opt-level = "z"`），单安装包约 12MB。

> 💡 **v1.0.2 完整功能**：自动重连、本地配置持久化、零拷贝 IPC、时间戳/方向显示、chunked memcpy RingBuffer（write_4KB 提升 ≈625×）、前端 LogPanel（F2 切换 + 级别/关键字过滤）、VID/PID 精准识别、UI 版本号动态同步，详见 [CHANGELOG](#-路线图) 与 [bench-v0.6.0.md](docs/bench-v0.6.0.md)。
>
> 📥 **用户直接下载**：GitHub Release 页面 `Assets` 区有现成的 `OhMySerial_1.0.2_x64-setup.exe`，双击安装即可使用，无需任何配置（首次启动自动建配置目录）。

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
├── src/                          # React 前端
│   ├── components/               # UI 组件
│   │   ├── Terminal.tsx          # Xterm.js 渲染（响应主题 + 时间戳 + 收/发方向）
│   │   ├── SerialToolbar.tsx     # 串口工具栏（三态指示灯 + 主题选择）
│   │   ├── SendPanel.tsx         # 发送面板（文本/HEX；onSent 回显 + forwardRef 暴露 focus/clear/send）
│   │   ├── PresetPanel.tsx       # 预设命令 CRUD
│   │   ├── StatusBar.tsx         # 状态栏（rAF 节流 15Hz 显示 TX/RX + 日志目录）
│   │   ├── HotkeyHelp.tsx        # 快捷键帮助浮层
│   │   └── LogPanel.tsx          # 抽屉式日志面板（级别/关键字过滤 + 打开目录）
│   ├── stores/                   # Zustand 状态管理（+ subscribeWithSelector middleware）
│   │   ├── serialStore.ts        # 串口连接 + sendData + Channel 注入回调 + 重连状态
│   │   ├── bufferStore.ts        # 收发字节统计（60Hz 累积）
│   │   ├── presetStore.ts        # 预设命令（持久化 v2）
│   │   ├── uiStore.ts            # 主题（持久化 + matchMedia）
│   │   ├── configStore.ts        # Rust 端配置镜像 + auto-save
│   │   └── logStore.ts           # 日志缓存 + 过滤
│   ├── hooks/                    # 自定义 hook
│   │   ├── useHotkeys.ts         # 全局快捷键 + matchHotkey / formatHotkey
│   │   ├── useThemeClasses.ts    # 主题 class 助手（DARK/LIGHT 语义集）
│   │   ├── useRafValue.ts        # rAF 节流 hook（纯函数 nextRafValue）
│   │   ├── useConfigSync.ts      # 多 store → configStore 同步 + debounce 500ms 写盘
│   │   └── useLogPolling.ts      # 2s 轮询拉取日志 + enabled 暂停
│   └── utils/                    # 工具函数
│       ├── hex.ts                # HEX 解析、CRC16
│       ├── encoding.ts           # GBK/UTF-8 编解码
│       ├── format.ts             # bytesToHuman
│       └── logParser.ts          # parseLogLine + levelAtLeast
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── serial/               # 串口驱动 + 64KB RingBuffer（chunked memcpy）
│   │   ├── ipc/commands.rs       # 21 个 Tauri IPC + Channel<Vec<u8>> 零拷贝
│   │   ├── sender/               # SendQueue + PreciseSender
│   │   ├── log_init.rs           # fern 文件日志 + 7 天清理 + read_recent_lines + parse_line
│   │   ├── config_impl.rs        # serde 配置 + 原子写 (tmp + rename)
│   │   └── error.rs              # SerialError + From<io::Error>
│   ├── benches/                  # criterion 性能基准（4 个）
│   ├── capabilities/             # Tauri 2.x 权限配置
│   └── tauri.conf.json
├── src-tauri/tests/              # Rust 集成测试（12 个）
├── tests/frontend/               # 前端测试（117 个）
├── docs/                         # 设计与实施计划 + 性能基准报告
│   ├── bench-v0.4.0.md           # v0.4.0 性能基准（byte-loop 基线）
│   └── bench-v0.6.0.md           # v0.6.0 性能基准（chunked memcpy + Channel 零拷贝）
└── README.md                     # 本文件
```

> 📝 `CLAUDE.md` 不入 Git（在 .gitignore），仅作长对话阶段性总结使用。

## 🧪 测试

### 前端测试（117 个）

```bash
npm test
```

涵盖：HEX 工具、bufferStore、serialStore 集成（mock Tauri API + Channel 注入）、bytesToHuman、uiStore（主题）、useHotkeys（matchHotkey / formatHotkey 纯函数）、useThemeClasses（DARK/LIGHT class 集合）、useRafValue（rAF 节流 + 纯函数）、Terminal（formatTimestamp + byteHex）、configStore（Rust 端配置同步）、logParser（parseLogLine + levelAtLeast）、logStore（applyFilter + setter）。

### Rust 单元测试（59 个）

```bash
cd src-tauri
cargo test --lib
```

涵盖：ring_buffer（含 6 个 chunked memcpy 边界测试）、send_queue、log_init（7 天清理 + parse_line + read_recent_lines）、reconnect（指数退避序列）。

### Rust 集成测试（12 个，需真实 CH340 硬件）

```powershell
# 接线：CH340 的 TX 与 RX 用杜邦线短接（GND 也接上）

# 设备管理器查看 COM 号，假设为 COM5
$env:OH_MY_SERIAL_TEST_PORT = "COM5"

cd src-tauri
cargo test --test env_check --test scenario_basic_echo --test scenario_large_transfer --test scenario_disconnect --test scenario_ipc_e2e --test scenario_polling
```

覆盖场景：环境检测、基础 echo、大数据量（8KB）、断线检测 + 重连、IPC E2E、SendQueue 轮询。

### 性能基准（criterion）

```bash
cd src-tauri
cargo bench --features bench
```

详见 [docs/bench-v0.6.0.md](docs/bench-v0.6.0.md)。v0.6.0 实测关键路径：

| 基准 | 延迟 | 吞吐 | v0.4.0 对比 |
|------|------|------|------|
| `ring_buffer_write/write_4KB` | 41.3 ns | **92.4 GiB/s** | ≈625× 提升 |
| `ring_buffer_cycle/write_256B_then_read` | 76.2 ns | 6.26 GiB/s | ≈50× 提升 |
| `drain_all/drain_4KB` | 143 ns | **26.6 GiB/s** | ≈410× 提升 |
| `send_queue/add_256_to_queue` | 3.11 µs | — | 持平 |

**CI 模式**：GitHub Actions 自动安装 com0com 虚拟串口对，无需硬件。

## 🗺 路线图

- [x] **v0.1.0** — 基础框架、IPC、环形缓冲区、Xterm.js 组件
- [x] **v0.2.0** — 数据接收打通 + 19 个集成测试
- [x] **v0.3.0** — 发送闭环（SendPanel + PresetPanel + SendQueue）+ 断线检测 + StatusBar
- [x] **v0.4.0** — 主题切换 + 快捷键 + PreciseSender 集成 + 文件日志 + 性能基准 + 应用图标 + 浅色模式可读性
- [x] **v0.5.0** — 本地配置持久化（config.json + IPC + auto-save）+ 自动重连（指数退避 1/2/4/8/15s 最多 5 次）
- [x] **v0.6.0** — Channel<Vec<u8>> 零拷贝 + RingBuffer chunked memcpy (~625× 提升) + 时间戳/收/发方向 + rAF 节流 + selector 订阅
- [x] **v1.0.0** — 前端 LogPanel（F2 切换 + 级别/关键字过滤 + 打开日志目录）+ Rust 端 read_recent_lines + 2 个新 IPC
- [x] **v1.0.1** — 终端去整行底色（前景色 RX/TX）+ VID/PID 精准识别（CH340/FTDI/CP210x/PL2303/MCP/XR21V/TUSB3410）+ 去掉无意义 `(Unknown)` 后缀 + 三处 version 字段对齐
- [x] **v1.0.2** — UI 版本号动态读取 package.json（StatusBar + Terminal 同步）+ GitHub Release 附 installer 资源 + 测试数校准

> 📌 v1.0.0 是当前计划的终点。后续按用户需求再开新任务（v1.0.1/v1.0.2 为 v1.0.0 之后的补丁发布）。

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

详细的开发上下文请参考 [docs/superpowers/plans/](docs/superpowers/plans/) 中的实施计划文档。

## 📚 设计文档

- [架构设计](docs/plans/2026-05-29-OhMySerial-design.md) — 数据流、背压策略、IPC 设计
- [实施计划](docs/plans/2026-05-29-OhMySerial-implementation.md) — 历史 Task 1-14 详细步骤
- [Task 12 集成测试计划](docs/superpowers/plans/2026-06-01-task12-integration-tests.md)
- [v0.4.0 性能基准报告](docs/bench-v0.4.0.md) — byte-loop RingBuffer 基线
- [v0.6.0 性能基准报告](docs/bench-v0.6.0.md) — chunked memcpy + Channel 零拷贝（write_4KB 提升 ≈625×）

## 📄 许可证

[MIT](LICENSE) © RoninQiu
