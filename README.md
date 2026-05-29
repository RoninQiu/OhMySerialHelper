# OhMySerial

使用AI开发：
高性能、轻量级串口调试助手，致力于作为传统 SSCOM 软件的现代化替代品。

## 核心特性

- **高性能**：Rust 异步内核 + WebGL 加速渲染，支持 921600 高波特率无卡顿
- **无损 HEX 切换**：收发原始字节流，文本/HEX 视图秒级无损切换
- **高精度定时**：定时发送下沉至 Rust 后端，不受后台限流影响
- **便携免装**：绿色单文件，Windows 10/11 即开即用

## 技术栈

| 层级 | 技术                         |
| ---- | ---------------------------- |
| 后端 | Rust + Tauri 2.x + Tokio     |
| 前端 | React + TypeScript + Zustand |
| 渲染 | Xterm.js (WebGL)             |
| 串口 | serialport                   |

## 功能预览

| 功能     | 说明                               |
| -------- | ---------------------------------- |
| 串口枚举 | 自动扫描 CH340, FTDI, CP210x 等    |
| 波特率   | 110 ~ 921600 及自定义              |
| 编码     | UTF-8 / GBK                        |
| 视图     | 文本 / HEX                         |
| 发送     | 文本发送 / HEX 发送 / 定时循环发送 |
| 预设命令 | 权重优先级管理                     |
| 日志     | 本地存储 + 自定义文件导出          |
| 主题     | 深色 / 浅色模式                    |

## 开发

### 环境要求

- Node.js 18+
- Rust 1.70+
- Windows 10/11

### 本地开发

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run tauri dev
```

### 项目结构

```
OhMySerialHelper/
├── src/                      # React 前端源码
│   ├── components/           # UI 组件
│   ├── stores/               # Zustand 状态管理
│   └── utils/                # 工具函数
├── src-tauri/                # Rust 后端源码
│   ├── src/
│   │   ├── main.rs           # 入口
│   │   └── lib.rs            # 核心逻辑
│   └── Cargo.toml
├── docs/
│   └── plans/                # 设计文档
└── README.md
```

## 构建

```bash
# 构建便携版单文件 exe
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/nsis/`。

## 设计文档

详细设计思路见 [设计文档](./docs/plans/2026-05-29-OhMySerial-design.md)。

## 许可证

MIT
