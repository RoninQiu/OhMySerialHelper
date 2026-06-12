/**
 * @vitest-environment jsdom
 *
 * fonts.ts 纯函数与常量单测
 */
import { describe, it, expect } from "vitest";
import {
  clampFontSize,
  resolveFontFamily,
  FONT_SIZE_RANGE,
  SYSTEM_DEFAULT_FAMILY,
  SYSTEM_DEFAULT_KEY,
} from "../../src/utils/fonts";

describe("clampFontSize", () => {
  it("11 → 12 (下界)", () => expect(clampFontSize(11)).toBe(12));
  it("25 → 24 (上界)", () => expect(clampFontSize(25)).toBe(24));
  it("14 不变", () => expect(clampFontSize(14)).toBe(14));
  it("12 边界不变", () => expect(clampFontSize(12)).toBe(12));
  it("24 边界不变", () => expect(clampFontSize(24)).toBe(24));
});

describe("resolveFontFamily", () => {
  it("'system-default' 返回 fallback 常量", () => {
    expect(resolveFontFamily("system-default")).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("空串当作 system-default", () => {
    expect(resolveFontFamily("")).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("null 当作 system-default", () => {
    expect(resolveFontFamily(null)).toBe(SYSTEM_DEFAULT_FAMILY);
  });
  it("普通字体名拼接 fallback 栈", () => {
    const result = resolveFontFamily("JetBrains Mono");
    expect(result).toContain("JetBrains Mono");
    expect(result).toContain(SYSTEM_DEFAULT_FAMILY);
  });
});

describe("FONT_SIZE_RANGE", () => {
  it("默认 14, 范围 12-24, 步进 2", () => {
    expect(FONT_SIZE_RANGE).toEqual({
      min: 12,
      max: 24,
      step: 2,
      default: 14,
    });
  });
});

describe("SYSTEM_DEFAULT_KEY", () => {
  it("等于 'system-default'", () => {
    expect(SYSTEM_DEFAULT_KEY).toBe("system-default");
  });
});