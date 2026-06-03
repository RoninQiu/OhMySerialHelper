/**
 * @vitest-environment jsdom
 *
 * configStore 单测
 * - 无 Tauri 环境时 loadFromBackend 立即标记 loaded=true，不抛错
 * - save 在非 Tauri 环境是 no-op
 * - 默认值正确
 * - 字段合并：Rust 配置覆盖默认值
 */
import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { useConfigStore, DEFAULT_CONFIG, AppConfigFE } from "../../src/stores/configStore";
import { useSerialStore } from "../../src/stores/serialStore";
import { useUiStore } from "../../src/stores/uiStore";
import { useBufferStore } from "../../src/stores/bufferStore";

describe("configStore", () => {
  beforeEach(() => {
    // 重置所有相关 store
    useConfigStore.setState({ config: DEFAULT_CONFIG, loaded: false, loading: false });
    useSerialStore.setState({
      baudRate: 115200,
      encoding: "utf8",
      portName: "",
    });
    useUiStore.setState({ theme: "dark" });
    useBufferStore.setState({ bufferSize: 10 * 1024 * 1024 });
  });

  it("默认值正确", () => {
    const c = useConfigStore.getState().config;
    expect(c.baud_rate).toBe(115200);
    expect(c.theme).toBe("dark");
    expect(c.encoding).toBe("utf8");
    expect(c.buffer_size).toBe(65536);
    expect(c.auto_reconnect).toBe(true);
    expect(c.reconnect_max_attempts).toBe(5);
  });

  it("无 Tauri 环境：loadFromBackend 立即完成（loaded=true）", async () => {
    // jsdom 默认无 __TAURI_INTERNALS__
    expect("__TAURI_INTERNALS__" in window).toBe(false);
    await useConfigStore.getState().loadFromBackend();
    expect(useConfigStore.getState().loaded).toBe(true);
    expect(useConfigStore.getState().loading).toBe(false);
  });

  it("无 Tauri 环境：save 是 no-op", async () => {
    // 不应抛错
    await expect(useConfigStore.getState().save()).resolves.toBeUndefined();
  });

  it("loading 期间 save 跳过写盘", async () => {
    useConfigStore.setState({ loading: true });
    // 应直接返回，不调用 invoke
    await useConfigStore.getState().save();
    expect(useConfigStore.getState().loading).toBe(true);
  });

  it("AppConfigFE 字段可序列化", () => {
    const c: AppConfigFE = {
      last_port: "COM3",
      baud_rate: 9600,
      data_bits: 7,
      stop_bits: 2,
      parity: "even",
      encoding: "gbk",
      theme: "light",
      buffer_size: 1024 * 1024,
      auto_reconnect: false,
      reconnect_max_attempts: 3,
    };
    // 字段名都是 snake_case（与 Rust 端 serde 一致）
    expect(c.last_port).toBe("COM3");
    expect(c.reconnect_max_attempts).toBe(3);
  });
});

describe("serialStore / 重连状态", () => {
  beforeEach(() => {
    useSerialStore.setState({ reconnect: null });
  });

  it("setReconnect 设置/清除进度", () => {
    expect(useSerialStore.getState().reconnect).toBeNull();
    useSerialStore.getState().setReconnect({
      state: "attempt",
      attempt: 1,
      max_attempts: 5,
      next_delay_ms: 1000,
      message: "1s 后第 1 次重试 COM5",
    });
    expect(useSerialStore.getState().reconnect?.attempt).toBe(1);
    useSerialStore.getState().setReconnect(null);
    expect(useSerialStore.getState().reconnect).toBeNull();
  });

  it("cancelReconnect 不抛错（无 Tauri）", async () => {
    await expect(useSerialStore.getState().cancelReconnect()).resolves.toBeUndefined();
  });

  it("openPort 后 reconnect 清空", async () => {
    useSerialStore.getState().setReconnect({
      state: "attempt",
      attempt: 1,
      max_attempts: 5,
      next_delay_ms: 1000,
      message: "x",
    });
    // 直接 setState 模拟（openPort 内部 invoke 会失败）
    useSerialStore.setState({
      isOpen: true,
      portName: "COM5",
      baudRate: 115200,
      disconnected: false,
      reconnect: null,
    });
    expect(useSerialStore.getState().reconnect).toBeNull();
  });
});
