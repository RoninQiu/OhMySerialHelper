/**
 * @vitest-environment jsdom
 *
 * terminalFormat 单测（v1.2.0）
 *
 * 用途：formatLine 生成去 ANSI 的纯文本行，用于录制文件
 *
 * 覆盖：
 * 1. HEX 视图 + RX 方向：[ts] ← [HEX] AA BB CC
 * 2. TEXT 视图 + TX 方向：[ts] → hello
 * 3. UTF-8 解码（含中文等非 ASCII）
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

// mock xterm 防止 Terminal.tsx 副作用
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => ({
    write: vi.fn(),
    writeLine: vi.fn(),
    open: vi.fn(),
    dispose: vi.fn(),
    onData: vi.fn(),
    loadAddon: vi.fn(),
  })),
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(() => ({
    fit: vi.fn(),
    activate: vi.fn(),
  })),
}));

import { formatLine } from "../../src/utils/terminalFormat";

describe("terminalFormat - formatLine", () => {
  beforeEach(() => {
    // 固定时间戳
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-22T14:35:12.456"));
  });

  it("HEX 视图 + RX：生成 '[ts] ← [HEX] AA BB CC' 格式", () => {
    const data = new Uint8Array([0xaa, 0xbb, 0xcc]);
    const line = formatLine(data, "rx", "hex", "utf8");
    expect(line).toBe("[14:35:12.456] ← [HEX] AA BB CC");
  });

  it("TEXT 视图 + TX：生成 '[ts] → hello' 格式（无 [HEX] 标签）", () => {
    const data = new TextEncoder().encode("hello");
    const line = formatLine(data, "tx", "text", "utf8");
    expect(line).toBe("[14:35:12.456] → hello");
  });

  it("UTF-8 解码：中文等多字节字符正确", () => {
    const data = new TextEncoder().encode("你好");
    const line = formatLine(data, "rx", "text", "utf8");
    expect(line).toBe("[14:35:12.456] ← 你好");
  });
});
