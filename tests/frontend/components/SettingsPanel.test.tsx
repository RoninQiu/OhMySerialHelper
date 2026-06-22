/**
 * @vitest-environment jsdom
 *
 * SettingsPanel 单测（v1.2.0）
 *
 * 不依赖 Tauri runtime（mock invoke），测试 Modal 行为：
 * 1. 渲染：默认路径字段 + toggle 字段
 * 2. "保存" 按钮调 configStore.save
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { createRoot, type Root } from "react-dom/client";
import { act } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => null),
  save: vi.fn(async () => null),
}));

import { SettingsPanel } from "../../../src/components/SettingsPanel";
import { useConfigStore, DEFAULT_CONFIG } from "../../../src/stores/configStore";

let root: Root;
let container: HTMLDivElement;

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);

  useConfigStore.setState({
    config: {
      ...DEFAULT_CONFIG,
      default_capture_path: "",
      prompt_save_dialog: true,
    },
  });
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe("SettingsPanel - 渲染字段", () => {
  it("open=true: 渲染默认路径输入框 + toggle + 保存按钮", () => {
    act(() => {
      root.render(<SettingsPanel open={true} onClose={() => {}} />);
    });
    // 路径输入框
    const inputs = container.querySelectorAll("input");
    expect(inputs.length).toBeGreaterThanOrEqual(2); // text + checkbox
    // 保存按钮
    const saveBtn = Array.from(container.querySelectorAll("button")).find(
      (b) => b.textContent?.trim() === "保存",
    );
    expect(saveBtn).toBeTruthy();
  });
});

describe("SettingsPanel - 保存", () => {
  it("点击保存按钮: 调 configStore.save", async () => {
    const saveSpy = vi.fn(async () => {});
    useConfigStore.setState({ save: saveSpy });

    act(() => {
      root.render(<SettingsPanel open={true} onClose={() => {}} />);
    });

    const saveBtn = Array.from(container.querySelectorAll("button")).find(
      (b) => b.textContent?.trim() === "保存",
    );
    expect(saveBtn).toBeTruthy();
    await act(async () => {
      saveBtn!.click();
    });
    expect(saveSpy).toHaveBeenCalledTimes(1);
  });
});
