/**
 * @vitest-environment jsdom
 *
 * Terminal 字号/字体单测（reviewer MAJOR #5）
 *
 * 注：本仓库不安装 @testing-library/react，所以组件的 React useEffect +
 * xterm 集成不在单测覆盖（由 tauri dev 手动验证）。这里测：
 * 1. resolveFontFamily 是 Terminal 字号/字体生效的纯函数依赖
 * 2. configStore 的 font_size / font_family 字段在 setFontSize/setFontFamily
 *    下的值变化，会被 Terminal 组件的 useEffect 读走
 * 3. 整个链路：setFontSize → store 更新 → resolveFontFamily 输出正确字符串
 */
import { describe, it, expect, beforeEach } from "vitest";
import { useConfigStore, DEFAULT_CONFIG } from "../../../src/stores/configStore";
import {
  resolveFontFamily,
  SYSTEM_DEFAULT_FAMILY,
  SYSTEM_DEFAULT_KEY,
} from "../../../src/utils/fonts";

describe("Terminal 字号/字体 (MAJOR #5)", () => {
  beforeEach(() => {
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_size: 14, font_family: SYSTEM_DEFAULT_KEY },
    });
  });

  it("setFontSize(20) 后 store.config.font_size === 20（Terminal useEffect 会读取）", () => {
    useConfigStore.getState().setFontSize(20);
    expect(useConfigStore.getState().config.font_size).toBe(20);
  });

  it("setFontFamily('JetBrains Mono') 后 resolveFontFamily 输出拼接 fallback 栈", () => {
    useConfigStore.getState().setFontFamily("JetBrains Mono");
    const fontFamily = useConfigStore.getState().config.font_family;
    const resolved = resolveFontFamily(fontFamily);
    expect(resolved).toContain("JetBrains Mono");
    expect(resolved).toContain(SYSTEM_DEFAULT_FAMILY);
  });

  it("fontFamily === 'system-default' 走 fallback 常量（无前缀拼接）", () => {
    useConfigStore.getState().setFontFamily("system-default");
    const fontFamily = useConfigStore.getState().config.font_family;
    expect(resolveFontFamily(fontFamily)).toBe(SYSTEM_DEFAULT_FAMILY);
  });
});
