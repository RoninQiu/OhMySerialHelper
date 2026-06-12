/**
 * @vitest-environment jsdom
 *
 * configStore 字号/字体 setter 单测（reviewer CRITICAL #1 修复）
 * - 字段全部在嵌套 config.font_size / config.font_family 下（snake_case）
 * - setter 内置 clamp + trim 防御
 * - 嵌套更新不丢其他字段
 */
import { describe, it, expect, beforeEach } from "vitest";
import { useConfigStore, DEFAULT_CONFIG } from "../../src/stores/configStore";

describe("configStore font actions (CRITICAL #1 修复)", () => {
  beforeEach(() => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        font_size: 14,
        font_family: "system-default",
      },
    });
  });

  it("DEFAULT_CONFIG 默认值正确", () => {
    expect(DEFAULT_CONFIG.font_size).toBe(14);
    expect(DEFAULT_CONFIG.font_family).toBe("system-default");
  });

  it("setFontSize(20) 正常设置", () => {
    useConfigStore.getState().setFontSize(20);
    expect(useConfigStore.getState().config.font_size).toBe(20);
  });

  it("setFontSize(100) clamp 到 24", () => {
    useConfigStore.getState().setFontSize(100);
    expect(useConfigStore.getState().config.font_size).toBe(24);
  });

  it("setFontSize(0) clamp 到 12", () => {
    useConfigStore.getState().setFontSize(0);
    expect(useConfigStore.getState().config.font_size).toBe(12);
  });

  it("setFontFamily('') fallback 到 'system-default'", () => {
    useConfigStore.getState().setFontFamily("");
    expect(useConfigStore.getState().config.font_family).toBe("system-default");
  });

  it("setFontFamily('  JetBrains  ') trim 空格", () => {
    useConfigStore.getState().setFontFamily("  JetBrains  ");
    expect(useConfigStore.getState().config.font_family).toBe("JetBrains");
  });

  it("setFontSize 不影响其他字段", () => {
    const before = { ...useConfigStore.getState().config };
    useConfigStore.getState().setFontSize(20);
    const after = useConfigStore.getState().config;
    expect(after.baud_rate).toBe(before.baud_rate);
    expect(after.theme).toBe(before.theme);
    expect(after.font_family).toBe(before.font_family);
    expect(after.font_size).toBe(20);
  });
});