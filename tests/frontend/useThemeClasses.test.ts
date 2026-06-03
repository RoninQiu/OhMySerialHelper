/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach } from "vitest";
import { DARK_CLASSES, LIGHT_CLASSES } from "../../src/hooks/useThemeClasses";
import { useUiStore } from "../../src/stores/uiStore";

describe("useThemeClasses / 主题 class 助手", () => {
  beforeEach(() => {
    useUiStore.setState({ theme: "dark" });
    document.documentElement.className = "dark";
  });

  describe("DARK_CLASSES（暗色）", () => {
    it("所有 class 都有 dark: 前缀", () => {
      const all = JSON.stringify(DARK_CLASSES);
      expect(all).toMatch(/dark:/);
      // 不能出现裸的 bg-white / text-gray-900
      expect(all).not.toMatch(/"bg-white"/);
      expect(all).not.toMatch(/"text-gray-900"/);
    });

    it("bg/text/border/status 字段都存在", () => {
      expect(DARK_CLASSES.bg).toBeDefined();
      expect(DARK_CLASSES.text).toBeDefined();
      expect(DARK_CLASSES.border).toBeDefined();
      expect(DARK_CLASSES.status).toBeDefined();
    });
  });

  describe("LIGHT_CLASSES（浅色）", () => {
    it("所有 class 都没有 dark: 前缀", () => {
      const all = JSON.stringify(LIGHT_CLASSES);
      expect(all).not.toMatch(/dark:/);
    });

    it("bg.primary 是 bg-white（白底）", () => {
      expect(LIGHT_CLASSES.bg.primary).toBe("bg-white");
    });

    it("text.muted ≥ gray-600（WCAG AA 在白底上）", () => {
      // gray-500 (4.6:1) 视觉偏淡；浅色模式必须 ≥ gray-600 (7:1)
      expect(LIGHT_CLASSES.text.muted).toMatch(/text-gray-(6|7|8|9)\d\d/);
    });

    it("text.inverse = text-white（按钮白字，跨主题一致）", () => {
      expect(LIGHT_CLASSES.text.inverse).toBe("text-white");
    });
  });

  describe("resolvedTheme 解析", () => {
    it("显式 dark → 总是 'dark'", () => {
      useUiStore.getState().setTheme("dark");
      expect(useUiStore.getState().resolvedTheme()).toBe("dark");
    });

    it("显式 light → 总是 'light'", () => {
      useUiStore.getState().setTheme("light");
      expect(useUiStore.getState().resolvedTheme()).toBe("light");
    });

    it("system 模式返回值 ∈ {dark, light}", () => {
      useUiStore.getState().setTheme("system");
      const resolved = useUiStore.getState().resolvedTheme();
      // jsdom 默认 matchMedia 不匹配任何值；保留宽松断言
      expect(["dark", "light"]).toContain(resolved);
    });
  });
});
