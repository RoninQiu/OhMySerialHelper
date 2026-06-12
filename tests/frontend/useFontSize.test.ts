/**
 * @vitest-environment jsdom
 *
 * useFontSize / applyFontSizeSync 单测
 *
 * 注：本仓库不安装 @testing-library/react（避免增加 react test-renderer
 * 体积），所以 hook 的 React useEffect 行为仅通过 ts 类型 + 集成测试保证；
 * 这里只测纯副作用函数 applyFontSizeSync 与 store → DOM 的桥接（手动调用
 * document.documentElement.style.fontSize 验证 useFontSize 的核心意图）。
 */
import { describe, it, expect, beforeEach } from "vitest";
import { applyFontSizeSync } from "../../src/hooks/useFontSize";
import { useConfigStore, DEFAULT_CONFIG } from "../../src/stores/configStore";
import { clampFontSize } from "../../src/utils/fonts";

describe("applyFontSizeSync", () => {
  beforeEach(() => {
    document.documentElement.style.fontSize = "";
  });

  it("默认参数同步设置 <html> fontSize 为 14", () => {
    applyFontSizeSync();
    expect(document.documentElement.style.fontSize).toBe("14px");
  });

  it("显式传参同步设置", () => {
    applyFontSizeSync(20);
    expect(document.documentElement.style.fontSize).toBe("20px");
  });

  it("与 store 的当前 font_size 保持一致（桥接验证）", () => {
    // 模拟 user 在 store 改字号 → 同步应用到 DOM
    useConfigStore.getState().setFontSize(18);
    applyFontSizeSync(useConfigStore.getState().config.font_size);
    expect(document.documentElement.style.fontSize).toBe("18px");
  });

  it("DEFAULT_CONFIG.font_size 等于 14（main.tsx 默认调用值）", () => {
    expect(DEFAULT_CONFIG.font_size).toBe(14);
  });

  it("clampFontSize 防越界：setFontSize(100) 后 applyFontSizeSync 写入 24px", () => {
    useConfigStore.getState().setFontSize(100);
    const clamped = clampFontSize(useConfigStore.getState().config.font_size);
    applyFontSizeSync(clamped);
    expect(document.documentElement.style.fontSize).toBe("24px");
  });
});