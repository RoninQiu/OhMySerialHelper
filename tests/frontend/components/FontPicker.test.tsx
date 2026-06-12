/**
 * @vitest-environment jsdom
 *
 * FontPicker Combobox 单测（reviewer MAJOR #6 补全）
 *
 * 注：本仓库不安装 @testing-library/react，所以这里用 react-dom 直接 render
 * 到 document.body，通过 querySelector 验证 DOM 结构 + store 副作用。
 *
 * 覆盖：
 * 1. 触发按钮存在 + 关闭时不显示 listbox
 * 2. 点击按钮打开 + 第 1 项"系统默认"
 * 3. 输入"jet"过滤列表
 * 4. 点击列表项写入 store + 关闭
 * 5. Esc 关闭
 * 6. 打开后 input 自动 focus（reviewer #4）
 * 7. 关闭后焦点回到触发按钮（reviewer #4）
 * 8. ArrowDown 移动高亮（MAJOR #6）
 * 9. ArrowUp 在第 1 项时不变（MAJOR #6 边界）
 * 10. Enter 选中当前高亮项（MAJOR #6）
 * 11. 空列表显示"未找到等宽字体"
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { createRoot, type Root } from "react-dom/client";
import { act } from "react";
import { FontPicker } from "../../../src/components/FontPicker";
import { useFontStore } from "../../../src/stores/fontStore";
import { useConfigStore, DEFAULT_CONFIG } from "../../../src/stores/configStore";
import { SYSTEM_DEFAULT_KEY } from "../../../src/utils/fonts";

let root: Root;
let container: HTMLDivElement;

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);

  useFontStore.setState({
    fonts: [
      { family: "Consolas" },
      { family: "JetBrains Mono" },
      { family: "Cascadia Code" },
    ],
    loaded: true,
  });
  useConfigStore.setState({
    config: { ...DEFAULT_CONFIG, font_family: SYSTEM_DEFAULT_KEY },
  });
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  container.remove();
});

function render() {
  act(() => {
    root.render(<FontPicker />);
  });
}

function tickRafs(n = 2) {
  return new Promise<void>((resolve) => {
    let count = 0;
    function step() {
      count++;
      if (count >= n) resolve();
      else requestAnimationFrame(step);
    }
    requestAnimationFrame(step);
  });
}

describe("FontPicker Combobox (MAJOR #6 补全)", () => {
  it("渲染触发按钮 + 关闭时不显示列表", () => {
    render();
    expect(container.querySelector("button[aria-haspopup='listbox']")).toBeTruthy();
    expect(container.querySelector("[role='listbox']")).toBeNull();
  });

  it("点击按钮打开列表 + 第 1 项是'系统默认'", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const listbox = container.querySelector("[role='listbox']")!;
    expect(listbox).toBeTruthy();
    expect(listbox.textContent).toContain("系统默认");
  });

  it("输入'jet' 过滤列表", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const input = container.querySelector("input[role='textbox']")! as HTMLInputElement;
    // React 跟踪 input value 用 native setter，必须绕开以触发 onChange
    const proto = Object.getPrototypeOf(input);
    const setter = Object.getOwnPropertyDescriptor(proto, "value")!.set!;
    act(() => {
      setter.call(input, "jet");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const items = container.querySelectorAll("[role='option']");
    expect(items.length).toBe(1);
    expect(items[0].textContent).toContain("JetBrains Mono");
  });

  it("点击列表项 → 写入 store + 关闭", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const items = container.querySelectorAll("[role='option']");
    // 第 0 项=系统默认, 第 1 项=Consolas, 第 2 项=JetBrains Mono, 第 3 项=Cascadia Code
    const jetbrains = Array.from(items).find((el) => el.textContent === "JetBrains Mono")!;
    act(() => {
      jetbrains.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(useConfigStore.getState().config.font_family).toBe("JetBrains Mono");
    expect(container.querySelector("[role='listbox']")).toBeNull();
  });

  it("Esc 关闭列表", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(container.querySelector("[role='listbox']")).toBeNull();
  });

  it("打开后 input 自动 focus（reviewer #4）", async () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await tickRafs(2);
    const input = container.querySelector("input[role='textbox']")!;
    expect(document.activeElement).toBe(input);
  });

  it("关闭后焦点回到触发按钮（reviewer #4）", async () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await tickRafs(2);
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    await tickRafs(2);
    expect(document.activeElement).toBe(trigger);
  });

  it("ArrowDown 移动高亮到第 2 项（MAJOR #6）", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const items = container.querySelectorAll("[role='option']");
    expect(items[0].className).toContain("bg-blue-50");
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    });
    // 重渲染后取最新 items
    const itemsAfter = container.querySelectorAll("[role='option']");
    expect(itemsAfter[1].className).toContain("bg-blue-50");
  });

  it("ArrowUp 在第 1 项时不变（MAJOR #6 边界）", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const items = container.querySelectorAll("[role='option']");
    expect(items[0].className).toContain("bg-blue-50");
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
    });
    const itemsAfter = container.querySelectorAll("[role='option']");
    expect(itemsAfter[0].className).toContain("bg-blue-50");
  });

  it("Enter 选中当前高亮项（MAJOR #6）", () => {
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    });
    expect(useConfigStore.getState().config.font_family).toBe("Consolas");
  });

  it("空列表显示'未找到等宽字体'提示", () => {
    useFontStore.setState({ fonts: [], loaded: true });
    render();
    const trigger = container.querySelector("button[aria-haspopup='listbox']")!;
    act(() => {
      trigger.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    // 列表默认会有"系统默认"占位项，要测"无匹配"提示需输入过滤词
    const input = container.querySelector("input[role='textbox']")! as HTMLInputElement;
    const proto = Object.getPrototypeOf(input);
    const setter = Object.getOwnPropertyDescriptor(proto, "value")!.set!;
    act(() => {
      setter.call(input, "xyznomatch");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect(container.textContent).toContain("未找到等宽字体");
  });
});
