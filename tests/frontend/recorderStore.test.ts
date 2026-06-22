/**
 * @vitest-environment jsdom
 *
 * recorderStore 单测（v1.2.0）
 *
 * 覆盖：
 * 1. startRecording：设置 isRecording=true + currentPath
 * 2. stopRecording：清空状态 + 返回 summary
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

// mock Tauri invoke：根据 cmd 返回对应 mock 数据
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, _args?: unknown) => {
    if (cmd === "cmd_stop_recording") {
      return { path: "/tmp/x.txt", bytes_written: 1024, duration_ms: 5000 };
    }
    return null;
  }),
}));

import { useRecorderStore } from "../../src/stores/recorderStore";

describe("recorderStore - startRecording", () => {
  beforeEach(() => {
    useRecorderStore.setState({
      isRecording: false,
      currentPath: null,
      bytesWritten: 0,
      startedAt: null,
    });
  });

  it("startRecording: 设置 isRecording=true + currentPath", async () => {
    await useRecorderStore.getState().startRecording("/tmp/cap.txt");
    const s = useRecorderStore.getState();
    expect(s.isRecording).toBe(true);
    expect(s.currentPath).toBe("/tmp/cap.txt");
    expect(s.startedAt).toBeGreaterThan(0);
  });
});

describe("recorderStore - stopRecording", () => {
  beforeEach(() => {
    useRecorderStore.setState({
      isRecording: true,
      currentPath: "/tmp/x.txt",
      bytesWritten: 1024,
      startedAt: Date.now(),
    });
  });

  it("stopRecording: 清空状态 + 返回 summary", async () => {
    const summary = await useRecorderStore.getState().stopRecording();
    const s = useRecorderStore.getState();
    expect(s.isRecording).toBe(false);
    expect(s.currentPath).toBeNull();
    expect(s.bytesWritten).toBe(0);
    expect(s.startedAt).toBeNull();
    expect(summary).toEqual({
      path: "/tmp/x.txt",
      bytes_written: 1024,
      duration_ms: expect.any(Number),
    });
  });
});
