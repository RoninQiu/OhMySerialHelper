/**
 * @vitest-environment jsdom
 *
 * useRafValue 纯逻辑单测
 * - 实际 hook 需要 React render + rAF mock，依赖 testing-library
 * - 这里只测核心纯函数 nextRafValue，hook 行为由 StatusBar 集成保证
 */
import { describe, it, expect } from "vitest";
import { nextRafValue } from "../../src/hooks/useRafValue";

describe("nextRafValue", () => {
  it("未到 frameSkip：counter 累加，值不变", () => {
    const r = nextRafValue(0, 10, 0, 4);
    expect(r.counter).toBe(1);
    expect(r.value).toBe(0);
  });

  it("到达 frameSkip：返回最新值，重置 counter", () => {
    const r = nextRafValue(0, 10, 3, 4);
    expect(r.counter).toBe(0);
    expect(r.value).toBe(10);
  });

  it("值未变时返回旧引用（避免无意义 setState）", () => {
    const obj = { a: 1 };
    const r = nextRafValue(obj, obj, 3, 4);
    expect(r.value).toBe(obj);
  });

  it("frameSkip=1 立即更新", () => {
    const r = nextRafValue(0, 5, 0, 1);
    expect(r.counter).toBe(0);
    expect(r.value).toBe(5);
  });

  it("高频连续调用只显示最终值（counter 视角的合并）", () => {
    // 模拟 source 5 6 7 8 9，counter 走到 4 时取 latest
    let prev = 0;
    let counter = 0;
    const updates: number[] = [];
    for (const src of [5, 6, 7, 8, 9]) {
      const r = nextRafValue(prev, src, counter, 4);
      counter = r.counter;
      if (r.value !== prev) {
        updates.push(r.value);
        prev = r.value;
      }
    }
    // 5 次调用里，counter 0→1→2→3→4（=frameSkip 触发更新）→1
    expect(updates).toEqual([8]);
  });

  it("字符串/对象引用稳定", () => {
    const s1 = "hello";
    const s2 = "world";
    const r = nextRafValue(s1, s2, 3, 4);
    expect(r.value).toBe(s2);
  });
});
