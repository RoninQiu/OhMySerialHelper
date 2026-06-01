/**
 * serialStore 集成测试
 *
 * 验证 Zustand store 通过 mock Tauri API 与 IPC 命令正确交互
 * 这是 UI 端最重要的状态管理测试
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

// mock Tauri 模块
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) =>
    import("./mocks/tauri").then((m) => m.mockInvoke(...args)),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) =>
    import("./mocks/tauri").then((m) => m.mockTauriApi.listen(...args)),
}));

import { mockInvoke, clearMockEvents } from "./mocks/tauri";
import { useSerialStore } from "../../src/stores/serialStore";

describe("serialStore 集成测试", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    clearMockEvents();
    useSerialStore.setState({
      isOpen: false,
      portName: "",
      baudRate: 115200,
      dataBits: 8,
      stopBits: 1,
      parity: "none",
    });
  });

  it("openPort 调用 cmd_open_port 并传齐 5 个参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSerialStore.getState().openPort("COM5", 115200, 8, 1, "none");

    expect(mockInvoke).toHaveBeenCalledWith("cmd_open_port", {
      portName: "COM5",
      baudRate: 115200,
      dataBits: 8,
      stopBits: 1,
      parity: "none",
    });
    expect(useSerialStore.getState().isOpen).toBe(true);
    expect(useSerialStore.getState().portName).toBe("COM5");
  });

  it("openPort 默认参数（5/6/7/8 数据位 1/2 停止位）", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    // 不传 dataBits/stopBits/parity，应使用默认值
    await useSerialStore.getState().openPort("COM3", 9600);
    expect(mockInvoke).toHaveBeenCalledWith("cmd_open_port", {
      portName: "COM3",
      baudRate: 9600,
      dataBits: 8,
      stopBits: 1,
      parity: "none",
    });
  });

  it("openPort 失败时抛出错误且 isOpen 保持 false", async () => {
    mockInvoke.mockRejectedValueOnce("打开串口失败: Access is denied");

    await expect(
      useSerialStore.getState().openPort("COM5", 115200),
    ).rejects.toBeTruthy();
    expect(useSerialStore.getState().isOpen).toBe(false);
  });

  it("closePort 调用 cmd_close_port 并重置 isOpen", async () => {
    useSerialStore.setState({ isOpen: true, portName: "COM5" });
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSerialStore.getState().closePort();

    expect(mockInvoke).toHaveBeenCalledWith("cmd_close_port");
    expect(useSerialStore.getState().isOpen).toBe(false);
  });

  it("setBaudRate 仅更新 store 状态，不发 IPC", async () => {
    mockInvoke.mockClear();
    useSerialStore.getState().setBaudRate(921600);
    expect(useSerialStore.getState().baudRate).toBe(921600);
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("setEncoding 仅更新 store 状态", () => {
    useSerialStore.getState().setEncoding("gbk");
    expect(useSerialStore.getState().encoding).toBe("gbk");
  });
});
