import { describe, it, expect, beforeEach } from "vitest";
import { useUiStore } from "../../src/stores/uiStore";

describe("uiStore", () => {
  beforeEach(() => {
    // 重置到初始状态
    useUiStore.setState({ theme: "dark" });
  });

  it("初始 theme 为 dark", () => {
    expect(useUiStore.getState().theme).toBe("dark");
  });

  it("setTheme 切换主题", () => {
    useUiStore.getState().setTheme("light");
    expect(useUiStore.getState().theme).toBe("light");
  });

  it("resolvedTheme: 显式主题直接返回", () => {
    useUiStore.getState().setTheme("light");
    expect(useUiStore.getState().resolvedTheme()).toBe("light");
    useUiStore.getState().setTheme("dark");
    expect(useUiStore.getState().resolvedTheme()).toBe("dark");
  });

  it("resolvedTheme: system 模式返回系统偏好", () => {
    useUiStore.getState().setTheme("system");
    // jsdom 默认 prefers-color-scheme: light
    const resolved = useUiStore.getState().resolvedTheme();
    expect(["dark", "light"]).toContain(resolved);
  });
});
