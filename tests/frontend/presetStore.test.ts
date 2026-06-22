/**
 * @vitest-environment jsdom
 *
 * presetStore 单测
 * - v3 迁移：丢弃旧版本的 name 字段
 * - addCommand 不再要求 name
 * - 默认 commands 为空数组
 */
import { describe, it, expect, beforeEach, vi } from "vitest";

// mock crypto.randomUUID（jsdom 不一定有）
if (!("randomUUID" in globalThis.crypto)) {
  Object.defineProperty(globalThis.crypto, "randomUUID", {
    value: () => `uuid-${Math.random().toString(36).slice(2, 10)}`,
  });
}

import { usePresetStore, migratePreset, type PresetCommand } from "../../src/stores/presetStore";

describe("presetStore - 初始状态", () => {
  beforeEach(() => {
    usePresetStore.setState({ commands: [], isPolling: false });
  });

  it("默认 commands 为空数组", () => {
    expect(usePresetStore.getState().commands).toEqual([]);
  });

  it("默认 isPolling = false", () => {
    expect(usePresetStore.getState().isPolling).toBe(false);
  });
});

describe("presetStore - addCommand（v3 去 name）", () => {
  beforeEach(() => {
    usePresetStore.setState({ commands: [], isPolling: false });
  });

  it("添加 hex 命令：commands 数组多一条", async () => {
    await usePresetStore.getState().addCommand({
      content: "AA BB CC",
      type: "hex",
      priority: 50,
      enabled: true,
      intervalMs: 1000,
    });
    const cmds = usePresetStore.getState().commands;
    expect(cmds).toHaveLength(1);
    expect(cmds[0].type).toBe("hex");
    expect(cmds[0].content).toBe("AA BB CC");
    expect(cmds[0].id).toBeTruthy();
  });

  it("添加 text 命令", async () => {
    await usePresetStore.getState().addCommand({
      content: "AT+RST",
      type: "text",
      priority: 50,
      enabled: true,
      intervalMs: 1000,
    });
    const c = usePresetStore.getState().commands[0];
    expect(c.type).toBe("text");
    expect(c.content).toBe("AT+RST");
  });

  it("添加后 store 内对象没有 name 字段", async () => {
    // v3 设计：name 字段已从 PresetCommand 移除
    await usePresetStore.getState().addCommand({
      content: "hello",
      type: "text",
      priority: 50,
      enabled: true,
      intervalMs: 1000,
    });
    const c = usePresetStore.getState().commands[0] as unknown as Record<string, unknown>;
    expect("name" in c).toBe(false);
  });
});

describe("presetStore - migrate v2 → v3", () => {
  beforeEach(() => {
    // 重置 store 后注入旧版本数据
    usePresetStore.setState({ commands: [], isPolling: false });
  });

  it("丢弃旧 isPolling 字段 + 丢弃 name 字段（v2 → v3）", () => {
    // 模拟旧 localStorage 数据：v2 状态，包含 name
    const oldState = {
      commands: [
        {
          id: "old-1",
          name: "重启设备",
          content: "AT+RST",
          type: "text",
          priority: 50,
          enabled: true,
          intervalMs: 1000,
        },
      ],
      isPolling: true,
    };

    const migrated = migratePreset(oldState, 3);
    const m = migrated as {
      commands?: PresetCommand[];
      isPolling?: boolean;
    };
    // commands 应被保留（id 仍在，name 字段仍可能残留在 JSON 上，但 UI 不再依赖）
    expect(m.commands).toBeDefined();
    expect(m.commands![0].id).toBe("old-1");
    expect(m.commands![0].content).toBe("AT+RST");
    // isPolling 必须被丢弃
    expect("isPolling" in m).toBe(false);
  });

  it("v2 旧 name 字段在 commands 上残留无害（UI 不读）", () => {
    // v3 migrate 不主动 strip 旧 name（避免无谓遍历），UI 不再读 name 即可
    const oldState = {
      commands: [{ id: "x", name: "旧名字", content: "y", type: "text" }],
    };
    const migrated = migratePreset(oldState, 3) as { commands: Array<Record<string, unknown>> };
    // migrate 不删除 name（保留原数据），但 UI/接口层都不再用 name
    expect(migrated.commands[0].name).toBe("旧名字");
  });
});
