/**
 * @vitest-environment jsdom
 *
 * 字体列表加载守卫（v1.1.1 补丁）
 *
 * 症状：用户反馈 FontPicker 只显示"系统默认"，没有其他字体可选。
 *
 * 根因：`useFontStore.loadFonts()` 在 fontStore.ts 里定义了，但**没有任何
 *        地方调用它**。FontPicker 渲染时只看 `useFontStore((s) => s.fonts)`，
 *        而 fonts 默认是空数组 → 只显示硬编码的"系统默认"那一项。
 *
 * 修复：App.tsx 启动时（useEffect mount）调一次 `useFontStore.getState().loadFonts()`。
 *
 * 本测试用 3 个守卫防止未来回归：
 * 1. App.tsx 必须 import `useFontStore`
 * 2. App.tsx 必须调 `useFontStore.getState().loadFonts()`
 * 3. fontStore 启动状态：fonts=[]  loaded=false（防止有人"修复"成默认值非空）
 */
import { describe, it, expect, beforeEach } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { useFontStore } from "../../src/stores/fontStore";

const ROOT = resolve(__dirname, "../..");
const read = (rel: string) => readFileSync(resolve(ROOT, rel), "utf8");

describe("字体列表加载守卫", () => {
  beforeEach(() => {
    // 重置 fontStore 到初始状态
    useFontStore.setState({ fonts: [], loaded: false });
  });

  it("App.tsx 必须 import useFontStore（启动时加载系统字体）", () => {
    const app = read("src/App.tsx");
    expect(app).toMatch(/import\s+\{\s*useFontStore\s*\}\s+from\s+["']\.\/stores\/fontStore["']/);
  });

  it("App.tsx 必须调 useFontStore...loadFonts()（v1.1.0 漏写导致字体列表永远空）", () => {
    const app = read("src/App.tsx");
    // 接受以下任意调用形式：
    //   useFontStore.getState().loadFonts()
    //   const { loadFonts } = useFontStore(); loadFonts();
    //   const { loadFonts } = useFontStore.getState(); loadFonts();
    expect(app, "App.tsx 未调 useFontStore...loadFonts()").toMatch(
      /useFontStore[\s\S]{0,60}\.loadFonts\(\s*\)/,
    );
  });

  it("fontStore 初始状态：fonts=[] loaded=false（防止有人改默认值绕过 loadFonts）", () => {
    const state = useFontStore.getState();
    expect(state.fonts).toEqual([]);
    expect(state.loaded).toBe(false);
  });
});
