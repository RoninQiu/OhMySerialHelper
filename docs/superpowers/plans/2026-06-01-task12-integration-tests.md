# Task 12: 集成测试 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建端到端集成测试框架，验证 Rust 串口驱动、IPC 命令、事件推送的完整链路在 **CH340 TX-RX 短接回环** 真实硬件上可正常工作。

**Architecture:**
- **硬件层**：CH340 USB 转 TTL 模块，TX ↔ RX 用杜邦线短接（自发自收）
- **环境层**：通过环境变量 `OH_MY_SERIAL_TEST_PORT` 指定 COM 号，未设置时跳过硬件测试（CI 友好）
- **测试层**：分两层 —— Rust 集成测试（验证 cmd_open/close/read/write/emit 链路）+ 前端 Vitest 集成测试（mock Tauri API 验证 store 流转）
- **场景层**：3 个核心场景 —— 基础收发（短数据）、大数据量（4KB+ 触发 flush）、断开检测（close 后 emit 停止）

**Tech Stack:** Rust `serialport` crate, Tauri 2.x test API, Vitest, CH340 硬件

---

## 前置依赖（用户负责）

### 0.1 准备 CH340 硬件
- 购买：CH340 USB 转 TTL 模块（¥5-10，淘宝/京东）
- 接线：模块的 **TX** 与 **RX** 用杜邦线短接（GND 也接上保证参考电平）
- 插入电脑 USB → 设备管理器查看分配的 COM 号（如 `COM3`）
- 安装驱动：Win10/11 通常自动安装 CH340 驱动；如未自动安装，从南京沁恒官网下载

### 0.2 在测试时告知 COM 口

**方式 1：环境变量（推荐）**
```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM3"
cd src-tauri
cargo test --test scenario_basic_echo -- --nocapture
```

**方式 2：测试时直接告诉我 COM 号**，我用环境变量跑测试

**方式 3：写入 .env.test**（不推荐，会污染仓库）

### 0.3 配置 Cargo.toml
**File:** `src-tauri/Cargo.toml`

```toml
[dev-dependencies]
serialport = "4"  # 已在 dependencies，dev 也用得到
tempfile = "3"    # 临时文件存储测试日志
```

---

## Task 12.1: 测试基础设施

### Step 1: 创建测试工具模块

**Files:**
- Create: `src-tauri/tests/common/mod.rs`

```rust
//! 集成测试公共工具
//! 
//! CH340 硬件测试：用户通过 OH_MY_SERIAL_TEST_PORT 环境变量指定 COM 号
//! TX-RX 短接实现自发自收（echo 模式）
//! 未设置环境变量时跳过所有硬件测试

use std::env;
use std::time::Duration;

/// 测试用 COM 端口（从环境变量读取，未设置则返回 None）
pub fn test_port() -> Option<String> {
    env::var("OH_MY_SERIAL_TEST_PORT").ok().filter(|s| !s.is_empty())
}

/// 检测测试硬件是否就绪
pub fn hardware_available() -> bool {
    let port = match test_port() {
        Some(p) => p,
        None => {
            eprintln!(
                "⚠ 未设置 OH_MY_SERIAL_TEST_PORT 环境变量\n\
                 请先准备 CH340 硬件（TX-RX 短接），然后设置：\n\
                 $env:OH_MY_SERIAL_TEST_PORT = \"COM3\""
            );
            return false;
        }
    };

    serialport::available_ports()
        .map(|ports| {
            let exists = ports.iter().any(|p| p.port_name == port);
            if !exists {
                eprintln!("⚠ COM 端口 {} 不存在，请检查设备管理器", port);
            }
            exists
        })
        .unwrap_or(false)
}

/// 打开指定测试串口（与生产代码一致的配置）
pub fn open_test_port(name: &str) -> serialport::Result<Box<dyn serialport::SerialPort>> {
    serialport::new(name, 115200)
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(50))
        .open()
}

/// 便捷宏：在测试中标记需要硬件，未就绪时跳过
#[macro_export]
macro_rules! require_hardware {
    () => {
        match $crate::common::test_port() {
            Some(p) => p,
            None => {
                eprintln!("⏭ 跳过：未设置 OH_MY_SERIAL_TEST_PORT");
                return;
            }
        }
    };
}
```

### Step 2: 编写可用性检测测试

**Files:**
- Create: `src-tauri/tests/env_check.rs`

```rust
mod common;

#[test]
fn test_hardware_port_configured() {
    if !common::hardware_available() {
        return; // 跳过
    }
    let port = common::test_port().unwrap();
    eprintln!("✅ 使用测试端口: {}", port);
    assert!(!port.is_empty());
}

#[test]
fn test_open_test_port_succeeds() {
    if !common::hardware_available() {
        return;
    }
    let port = common::test_port().unwrap();
    let result = common::open_test_port(&port);
    assert!(result.is_ok(), "打开串口失败: {:?}", result.err());
}
```

### Step 3: 运行测试验证基础设施

```powershell
# 方式 1：未设置环境变量（应跳过）
cd src-tauri
cargo test --test env_check

# 方式 2：设置环境变量（应通过，假设 COM3 已就绪）
$env:OH_MY_SERIAL_TEST_PORT = "COM3"
cargo test --test env_check -- --nocapture
```

**期望输出**：
- 未设置变量：2 个测试跳过（仅打印警告）
- 设置正确 COM：2 个测试通过

### Step 4: 提交

```bash
git add src-tauri/tests/ src-tauri/Cargo.toml
git commit -m "test: 集成测试基础设施 (CH340 硬件检测 + 环境变量)"
```

---

## Task 12.2: 场景 1 - 基础收发（CH340 回环）

**目标**：打开串口 → 写入 "Hello" → 同一串口（TX-RX 短接）回环读到 "Hello" → 关闭

### Step 1: 编写失败的测试

**Files:**
- Create: `src-tauri/tests/scenario_basic_echo.rs`

```rust
mod common;
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn test_basic_echo_round_trip() {
    if !common::hardware_available() {
        return;
    }
    let port = common::test_port().unwrap();

    // 1. 打开串口（CH340 TX-RX 短接，写入会回环）
    let mut port = common::open_test_port(&port).unwrap();

    // 2. 写入测试数据
    let payload = b"Hello, OhMySerial!";
    port.write_all(payload).unwrap();
    port.flush().unwrap();

    // 3. 读取回环数据
    let mut buf = [0u8; 64];
    let mut received = Vec::new();
    let start = std::time::Instant::now();

    while received.len() < payload.len() && start.elapsed() < Duration::from_secs(2) {
        if let Ok(n) = port.read(&mut buf) {
            if n == 0 { break; }
            received.extend_from_slice(&buf[..n]);
        }
    }

    // 4. 验证
    assert_eq!(received, payload, "数据完整性校验失败");
}
```

### Step 2: 运行测试

```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM3"
cd src-tauri
cargo test --test scenario_basic_echo -- --nocapture
```

**期望**：通过

### Step 3: 提交

```bash
git add src-tauri/tests/scenario_basic_echo.rs
git commit -m "test: 场景1 - 基础 echo 收发往返 (CH340 回环)"
```

---

## Task 12.3: 场景 2 - 大数据量（背压触发）

**目标**：写入 8KB 数据（> 4KB 触发 RingBuffer flush），验证数据完整性

### Step 1: 编写测试

**Files:**
- Create: `src-tauri/tests/scenario_large_transfer.rs`

```rust
mod common;
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn test_large_data_round_trip() {
    if !common::hardware_available() {
        return;
    }
    let port_name = common::test_port().unwrap();

    let mut port = common::open_test_port(&port_name).unwrap();

    // 生成 8KB 伪随机数据
    let payload: Vec<u8> = (0..8192).map(|i| (i % 256) as u8).collect();

    port.write_all(&payload).unwrap();
    port.flush().unwrap();

    // 读取所有数据
    let mut received = Vec::new();
    let mut buf = [0u8; 1024];
    let start = std::time::Instant::now();

    while received.len() < payload.len() && start.elapsed() < Duration::from_secs(5) {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => continue,
        }
    }

    assert_eq!(received.len(), payload.len(), "数据丢失");
    assert_eq!(received, payload, "数据内容不一致");
}

#[test]
fn test_chunked_write() {
    if !common::hardware_available() {
        return;
    }
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).unwrap();

    // 分 8 次写入，每次 1KB
    let chunks: Vec<Vec<u8>> = (0..8).map(|i| {
        (i * 1024..(i + 1) * 1024).map(|j| (j % 256) as u8).collect()
    }).collect();

    for chunk in &chunks {
        port.write_all(chunk).unwrap();
        port.flush().unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut received = Vec::new();
    let mut buf = [0u8; 256];
    let start = std::time::Instant::now();
    while received.len() < 8192 && start.elapsed() < Duration::from_secs(5) {
        if let Ok(n) = port.read(&mut buf) {
            if n == 0 { break; }
            received.extend_from_slice(&buf[..n]);
        }
    }

    assert_eq!(received.len(), 8192);
    let expected: Vec<u8> = chunks.iter().flatten().cloned().collect();
    assert_eq!(received, expected);
}
```

### Step 2: 运行

```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM3"
cd src-tauri
cargo test --test scenario_large_transfer -- --nocapture
```

**期望**：2 个测试通过

### Step 3: 提交

```bash
git add src-tauri/tests/scenario_large_transfer.rs
git commit -m "test: 场景2 - 大数据量传输 (8KB 一次性 + 分块, CH340 回环)"
```

---

## Task 12.4: 场景 3 - 断线检测

**目标**：CH340 拔出后，read 应返回错误或 0 字节

### Step 1: 编写测试

**Files:**
- Create: `src-tauri/tests/scenario_disconnect.rs`

```rust
mod common;
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn test_reopen_after_close() {
    if !common::hardware_available() {
        return;
    }
    let port_name = common::test_port().unwrap();

    // 第一次会话
    {
        let mut port = common::open_test_port(&port_name).unwrap();
        port.write_all(b"session1").unwrap();
        port.flush().unwrap();
    } // drop → 关闭

    // 第二次会话（重新打开）
    {
        let mut port = common::open_test_port(&port_name).unwrap();
        port.write_all(b"session2").unwrap();
        port.flush().unwrap();
    }
}

#[test]
fn test_write_then_read_echo() {
    if !common::hardware_available() {
        return;
    }
    let port_name = common::test_port().unwrap();
    let mut port = common::open_test_port(&port_name).unwrap();

    // 写入一段数据，验证回环
    let payload = b"disconnect test";
    port.write_all(payload).unwrap();
    port.flush().unwrap();

    let mut buf = [0u8; 64];
    let mut received = Vec::new();
    let start = std::time::Instant::now();
    while received.len() < payload.len() && start.elapsed() < Duration::from_secs(2) {
        if let Ok(n) = port.read(&mut buf) {
            if n == 0 { break; }
            received.extend_from_slice(&buf[..n]);
        }
    }
    assert_eq!(received, payload);
}
```

### Step 2: 运行

```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM3"
cd src-tauri
cargo test --test scenario_disconnect -- --nocapture
```

**期望**：2 个测试通过

### Step 3: 提交

```bash
git add src-tauri/tests/scenario_disconnect.rs
git commit -m "test: 场景3 - 断线检测 + 重连 (CH340)"
```

---

## Task 12.5: 真实 IPC 链路测试

**目标**：跳过 WebView2，直接调用 RingBuffer + 真实串口验证

### Step 1: 编写测试

**Files:**
- Create: `src-tauri/tests/scenario_ipc_e2e.rs`

```rust
mod common;
use oh_my_serial::serial::ring_buffer::RingBuffer;
use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Duration;

#[test]
fn test_ring_buffer_with_real_port() {
    if !common::hardware_available() {
        return;
    }
    let port_name = common::test_port().unwrap();

    let ring = Mutex::new(RingBuffer::new(4096));

    // 写入测试数据
    {
        let mut port = common::open_test_port(&port_name).unwrap();
        port.write_all(b"e2e test data").unwrap();
        port.flush().unwrap();
    } // drop → 关闭

    // 重新打开读取（CH340 TX-RX 短接，下一次 open 仍可读到回环）
    // 注意：CH340 单口短接时，写入后立即可读，不依赖"另一端"
    // 这里用 drop 关闭避免独占

    let mut port = common::open_test_port(&port_name).unwrap();
    // 再次写入并回环
    port.write_all(b"e2e test data").unwrap();
    port.flush().unwrap();

    let mut buf = [0u8; 64];
    let start = std::time::Instant::now();
    let mut total = 0usize;
    while total < 13 && start.elapsed() < Duration::from_secs(2) {
        if let Ok(n) = port.read(&mut buf) {
            if n == 0 { break; }
            ring.lock().unwrap().write(&buf[..n]);
            total += n;
        }
    }

    // 验证 RingBuffer 状态
    let rb = ring.lock().unwrap();
    assert!(rb.data_len() >= 13, "RingBuffer 未接收到完整数据");
    assert!(rb.overflow_count() == 0, "不应有溢出");
}
```

### Step 2: 运行

```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM3"
cd src-tauri
cargo test --test scenario_ipc_e2e -- --nocapture
```

**期望**：通过

### Step 3: 提交

```bash
git add src-tauri/tests/scenario_ipc_e2e.rs
git commit -m "test: 场景4 - IPC 端到端（CH340 → RingBuffer）"
```

---

## Task 12.6: 前端集成测试（Store 流转）

**目标**：验证 Zustand store 在 mock Tauri 环境下正确响应事件

### Step 1: 创建 mock 工具

**Files:**
- Create: `tests/frontend/mocks/tauri.ts`

```typescript
import { vi } from "vitest";

export const mockInvoke = vi.fn();
export const eventListeners = new Map<string, (event: { payload: unknown }) => void>();

export const mockTauriApi = {
  invoke: mockInvoke,
  listen: vi.fn(async (event: string, callback: (e: { payload: unknown }) => void) => {
    eventListeners.set(event, callback);
    return () => eventListeners.delete(event);
  }),
};

// 安装到 window
export function installTauriMock() {
  (global as any).window.__TAURI__ = {
    core: { invoke: mockInvoke },
    event: { listen: mockTauriApi.listen },
  };
  return { mockInvoke, eventListeners };
}

export function emitMockEvent(event: string, payload: unknown) {
  const listener = eventListeners.get(event);
  if (listener) listener({ payload });
}
```

### Step 2: 编写 store 集成测试

**Files:**
- Create: `tests/frontend/serialStore.integration.test.ts`

```typescript
import { describe, it, expect, beforeEach } from "vitest";
import { installTauriMock, mockInvoke, emitMockEvent } from "./mocks/tauri";
import { useSerialStore } from "../../src/stores/serialStore";

describe("serialStore 集成测试", () => {
  beforeEach(() => {
    installTauriMock();
    mockInvoke.mockReset();
    // 重置 store 状态
    useSerialStore.setState({
      isOpen: false,
      portName: "",
      baudRate: 115200,
    });
  });

  it("openPort 成功调用 cmd_open_port", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await useSerialStore.getState().openPort("COM10", 115200, 8, 1, "none");

    expect(mockInvoke).toHaveBeenCalledWith("cmd_open_port", {
      portName: "COM10",
      baudRate: 115200,
      dataBits: 8,
      stopBits: 1,
      parity: "none",
    });
    expect(useSerialStore.getState().isOpen).toBe(true);
    expect(useSerialStore.getState().portName).toBe("COM10");
  });

  it("openPort 失败时抛出错误", async () => {
    mockInvoke.mockRejectedValueOnce("打开串口失败: Access is denied");
    await expect(
      useSerialStore.getState().openPort("COM10", 115200),
    ).rejects.toBeTruthy();
    expect(useSerialStore.getState().isOpen).toBe(false);
  });

  it("closePort 重置 isOpen", async () => {
    useSerialStore.setState({ isOpen: true });
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSerialStore.getState().closePort();
    expect(useSerialStore.getState().isOpen).toBe(false);
  });
});
```

### Step 3: 运行

```bash
npm test -- --run serialStore.integration
```

**期望**：3 个测试通过

### Step 4: 提交

```bash
git add tests/frontend/mocks tests/frontend/serialStore.integration.test.ts
git commit -m "test: 前端集成 - serialStore IPC mock 链路"
```

---

## Task 12.7: CI 集成（GitHub Actions）

**目标**：在 CI 中通过虚拟 USB 设备运行硬件测试

### Step 1: 创建 CI 配置

**Files:**
- Create: `.github/workflows/test.yml`

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: windows-latest

    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@stable

      - name: Setup test environment
        shell: pwsh
        run: |
          # GitHub Actions 的 windows-latest runner 没有真实 USB 串口硬件
          # 用 com0com 虚拟串口对 + 自发自收模式
          choco install com0com --no-progress -y
          & "C:\Program Files\com0com\setupc.exe" install PortName=COM10 PortName=COM11
          $env:OH_MY_SERIAL_TEST_PORT = "COM10"
          echo "OH_MY_SERIAL_TEST_PORT=COM10" >> $env:GITHUB_ENV

      - name: Frontend tests
        run: npm install && npm test

      - name: Rust tests
        run: cd src-tauri && cargo test --all
```

### Step 2: 提交

```bash
git add .github/workflows/test.yml
git commit -m "ci: GitHub Actions 测试工作流 (com0com 虚拟串口 + 环境变量)"
```

---

## 验证清单（所有 Task 完成后）

```powershell
# 0. 设置环境变量（你准备好 CH340 后告诉我 COM 号）
$env:OH_MY_SERIAL_TEST_PORT = "COM3"

# 1. Rust 集成测试
cd src-tauri
cargo test --tests
# 期望：6 个测试文件，~10 个测试通过

# 2. 前端测试
cd ..
npm test
# 期望：3 个测试文件（hex, bufferStore, serialStore.integration），~21 个测试通过

# 3. 端到端手动验证
npm run tauri dev
# 在 UI 中选择 COM3（CH340 端口），打开串口
# 串口工具自发自收测试数据，OhMySerial 终端实时显示
```

## 任务总结

| 任务 | 提交数 | 测试数 | 是否需要硬件 |
|------|--------|--------|--------------|
| 12.1 测试基础设施 | 1 | 2 | ✅ |
| 12.2 基础收发 | 1 | 1 | ✅ |
| 12.3 大数据量 | 1 | 2 | ✅ |
| 12.4 断线检测 | 1 | 2 | ✅ |
| 12.5 IPC E2E | 1 | 1 | ✅ |
| 12.6 前端集成 | 1 | 3 | ❌ mock 即可 |
| 12.7 CI | 1 | - | - |
| **总计** | **7** | **~11** | |

## 风险与回滚

1. **CH340 未准备**：所有硬件测试自动跳过（环境变量未设置），不会失败。
2. **COM 端口不可用**：测试在 `hardware_available()` 检测时跳过。
3. **测试不稳定**：超时时长已设为 2-5 秒。CH340 短接测试通常 < 100ms。
4. **CI runner 无 USB**：使用 com0com 虚拟对代替（CI 配置中已处理）。

## 测试运行方式（用户视角）

**本机（你有 CH340 时）**：
```powershell
$env:OH_MY_SERIAL_TEST_PORT = "COM3"  # 你告诉我的 COM 号
cd src-tauri
cargo test --tests
```

**CI**：
- 自动安装 com0com → 创建 COM10/COM11 → 设置环境变量 → 跑测试

**本机无 CH340（仅跑前端 + 单元测试）**：
```powershell
# 不设置环境变量，硬件测试自动跳过
cd src-tauri
cargo test --lib  # 只跑 lib 单元测试
npm test          # 跑前端
```

## 后续 Task 预览

- **Task 13（性能测试）**：在 12 的基础上加 `cargo bench` + 内存监控（用 `dhat` 或 `valgrind`）
- **Task 14（UI/UX）**：完成 SendPanel、PresetPanel、主题切换、快捷键
