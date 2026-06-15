/**
 * @vitest-environment jsdom
 *
 * 字号调节 → UI 布局守卫测试（v1.1.0 补丁）
 *
 * 用户诉求：调节字号只影响 xterm 终端内的字符，**不**影响 UI 任何元素的
 * 物理尺寸，**不**导致整个应用出现横向滚动条。
 *
 * 本测试用 4 个静态守卫防止未来回归：
 * 1. App.tsx 根容器 className 必须含 `overflow-hidden`
 *    （防止任何 flex 子项意外撑大导致整页横向滚动条）
 * 2. StatusBar.tsx 根容器 className **不得**含 `overflow-x-auto`
 *    （防止内容溢出时显示水平滚动条——应该 truncate）
 * 3. useConfigStore.setFontSize 不应触碰 <html> 的 style.fontSize
 *    （防止有人加回 useFontSize / document.documentElement.style.fontSize）
 * 4. main.tsx 不应再 import 或调用 useFontSize / applyFontSizeSync
 *    （防止字号联动 <html>，让 Tailwind rem 跟着放大整个 UI）
 */
import { describe, it, expect, beforeEach } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { useConfigStore, DEFAULT_CONFIG } from "../../src/stores/configStore";

const ROOT = resolve(__dirname, "../..");
const read = (rel: string) => readFileSync(resolve(ROOT, rel), "utf8");

describe("字号调节 → UI 布局守卫", () => {
  beforeEach(() => {
    // 每次前重置 store 字号到 14
    useConfigStore.setState({
      config: { ...DEFAULT_CONFIG, font_size: 14 },
    });
    // 防止上一次测试残留
    document.documentElement.style.fontSize = "";
  });

  it("App.tsx 根容器必须有 overflow-hidden（防御整页横向滚动条）", () => {
    const app = read("src/App.tsx");
    // 取根 div 的 className（h-screen 那一行）
    const rootMatch = app.match(/<div\s+className=\{`flex flex-col h-screen[^`]*`}/);
    expect(rootMatch, "App.tsx 根 div 模板字符串未找到").toBeTruthy();
    expect(rootMatch![0]).toMatch(/overflow-hidden/);
  });

  it("StatusBar.tsx 根容器必须用 overflow-hidden，不得用 overflow-x-auto", () => {
    const sb = read("src/components/StatusBar.tsx");
    // 取根 div className
    const rootMatch = sb.match(
      /<div\s+className=\{`flex items-center justify-between[^`]*`}/,
    );
    expect(rootMatch, "StatusBar.tsx 根 div 模板字符串未找到").toBeTruthy();
    expect(rootMatch![0]).toMatch(/overflow-hidden/);
    expect(rootMatch![0]).not.toMatch(/overflow-x-auto/);
  });

  it("useConfigStore.setFontSize 不应修改 <html> fontSize", () => {
    useConfigStore.getState().setFontSize(24);
    expect(document.documentElement.style.fontSize).toBe("");
  });

  it("main.tsx 不应 import 或调用 useFontSize / applyFontSizeSync", () => {
    const main = read("src/main.tsx");
    expect(main).not.toMatch(/useFontSize/);
    expect(main).not.toMatch(/applyFontSizeSync/);
    expect(main).not.toMatch(/document\.documentElement\.style\.fontSize/);
  });

  it("setFontSize 只更新 store.font_size，不污染其它字段", () => {
    const before = { ...useConfigStore.getState().config };
    useConfigStore.getState().setFontSize(20);
    const after = useConfigStore.getState().config;
    expect(after.font_size).toBe(20);
    // 其它字段全部保持
    expect(after.last_port).toBe(before.last_port);
    expect(after.baud_rate).toBe(before.baud_rate);
    expect(after.theme).toBe(before.theme);
    expect(after.font_family).toBe(before.font_family);
  });
});
