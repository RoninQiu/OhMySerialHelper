/**
 * @vitest-environment jsdom
 *
 * PresetPanel 单测（v3 改造：去 name + 接 onSent + content 预览）
 *
 * 覆盖：
 * 1. 列表行展示 content 预览，不是 name
 * 2. 表单无 name 输入框（只有 type + content）
 * 3. 点击发送：调 sendData(bytes) + onSent(bytes)
 * 4. HEX 类型列表行用紫色徽章；TEXT 用蓝色徽章
 * 5. 串口未开时发送按钮 disabled
 * 6. 串口未开时点击不调 sendData / onSent
 * 7. 添加按钮：HEX 校验非法时显示错误，不写入 store
 * 8. 添加按钮：成功后清空表单
 * 9. 删除按钮：调 deleteCommand
 *
 * 不安装 @testing-library/react，react-dom 直接 render
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { createRoot, type Root } from "react-dom/client";
import { act } from "react";
import { PresetPanel } from "../../../src/components/PresetPanel";
import { usePresetStore, type PresetCommand } from "../../../src/stores/presetStore";
import { useSerialStore } from "../../../src/stores/serialStore";
import { useUiStore } from "../../../src/stores/uiStore";
import { useConfigStore, DEFAULT_CONFIG } from "../../../src/stores/configStore";

let root: Root;
let container: HTMLDivElement;
let sendDataMock: ReturnType<typeof vi.fn>;
let onSentMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);

  // 重置 store
  usePresetStore.setState({ commands: [], isPolling: false });
  useSerialStore.setState({
    isOpen: true, // 默认开启，便于测试发送路径
    disconnected: false,
    portName: "COM5",
  });
  useUiStore.setState({ theme: "dark" });
  useConfigStore.setState({ config: { ...DEFAULT_CONFIG } });

  // mock serialStore.sendData
  sendDataMock = vi.fn(async () => undefined);
  useSerialStore.setState({ sendData: sendDataMock as never });

  onSentMock = vi.fn();
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  container.remove();
});

function render() {
  act(() => {
    root.render(<PresetPanel onSent={onSentMock} />);
  });
}

function getInputByPlaceholder(text: string) {
  return Array.from(container.querySelectorAll("input,textarea")).find(
    (el) => (el as HTMLInputElement).placeholder === text,
  ) as HTMLInputElement | HTMLTextAreaElement | undefined;
}

function getButtonByText(text: string) {
  return Array.from(container.querySelectorAll("button")).find(
    (b) => b.textContent?.trim() === text,
  ) as HTMLButtonElement | undefined;
}

/** React 18 controlled input 需要走原生 setter 才能触发 onChange */
function setNativeValue(
  el: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement,
  value: string,
) {
  let proto: { value: string };
  let eventName: string;
  if (el instanceof HTMLTextAreaElement) {
    proto = HTMLTextAreaElement.prototype;
    eventName = "input";
  } else if (el instanceof HTMLSelectElement) {
    proto = HTMLSelectElement.prototype;
    eventName = "change";
  } else {
    proto = HTMLInputElement.prototype;
    eventName = "input";
  }
  const setter = Object.getOwnPropertyDescriptor(proto, "value")!.set!;
  setter.call(el, value);
  el.dispatchEvent(new Event(eventName, { bubbles: true }));
}

describe("PresetPanel - v3 列表展示", () => {
  it("列表行展示 content 预览（不是 name）", () => {
    // 注入一条预设
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "AA BB CC DD",
            type: "hex",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    // 列表项的 span 应展示 content 预览
    const listItem = container.querySelector("[data-preset-row]");
    const span = listItem?.querySelector("span[title]") as HTMLSpanElement | null;
    // title 是完整 content，可见文本是 content 预览
    expect(span).toBeTruthy();
    expect(span?.getAttribute("title")).toBe("AA BB CC DD");
    expect(span?.textContent).toContain("AA BB CC");
  });

  it("HEX 类型：紫色徽章", () => {
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "DE AD",
            type: "hex",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const badge = container.querySelector("[data-preset-row] span");
    expect(badge?.className).toMatch(/purple/);
    expect(badge?.textContent).toBe("HEX");
  });

  it("TEXT 类型：蓝色徽章", () => {
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "AT+RST",
            type: "text",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const badge = container.querySelector("[data-preset-row] span");
    expect(badge?.className).toMatch(/blue/);
    expect(badge?.textContent).toBe("TEXT");
  });
});

describe("PresetPanel - v3 表单（无 name）", () => {
  it("表单无 name 输入框", () => {
    render();
    const nameInput = getInputByPlaceholder("名称");
    expect(nameInput).toBeUndefined();
  });

  it("表单只有 type 选择 + content 输入", () => {
    render();
    const contentTa = container.querySelector("textarea");
    expect(contentTa).toBeTruthy();
    const typeSelect = container.querySelector("select") as HTMLSelectElement | null;
    expect(typeSelect).toBeTruthy();
    expect(Array.from(typeSelect!.options).map((o) => o.value)).toEqual(["text", "hex"]);
  });
});

describe("PresetPanel - 发送行为", () => {
  it("点击发送：调 sendData(bytes) + onSent(bytes)", async () => {
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "hi",
            type: "text",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const sendBtn = container.querySelector("[data-preset-row] button") as HTMLButtonElement;
    expect(sendBtn.textContent?.trim()).toBe("发送");

    await act(async () => {
      sendBtn.click();
    });
    // 等待 microtask
    await act(async () => {
      await Promise.resolve();
    });

    expect(sendDataMock).toHaveBeenCalledTimes(1);
    const sentBytes = sendDataMock.mock.calls[0][0] as Uint8Array;
    expect(Array.from(sentBytes)).toEqual([0x68, 0x69]); // "hi"

    // onSent 也被调，bytes 一致
    expect(onSentMock).toHaveBeenCalledTimes(1);
    const onSentBytes = onSentMock.mock.calls[0][0] as Uint8Array;
    expect(Array.from(onSentBytes)).toEqual([0x68, 0x69]);
  });

  it("HEX 类型发送：bytes 解析自 hexToBytes", async () => {
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "AA 11 22",
            type: "hex",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const sendBtn = container.querySelector("[data-preset-row] button") as HTMLButtonElement;
    await act(async () => {
      sendBtn.click();
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(sendDataMock).toHaveBeenCalledTimes(1);
    expect(Array.from(sendDataMock.mock.calls[0][0] as Uint8Array)).toEqual([0xaa, 0x11, 0x22]);
  });

  it("串口未开：发送按钮 disabled + 点击不调 sendData/onSent", async () => {
    useSerialStore.setState({ isOpen: false });
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "x",
            type: "text",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const sendBtn = container.querySelector("[data-preset-row] button") as HTMLButtonElement;
    expect(sendBtn.disabled).toBe(true);

    await act(async () => {
      sendBtn.click();
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(sendDataMock).not.toHaveBeenCalled();
    expect(onSentMock).not.toHaveBeenCalled();
  });

  it("sendData 抛错：错误信息含命令 content 前缀，不调 onSent", async () => {
    sendDataMock.mockRejectedValueOnce(new Error("串口未打开"));
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "1",
            content: "AT+RST",
            type: "text",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const sendBtn = container.querySelector("[data-preset-row] button") as HTMLButtonElement;
    await act(async () => {
      sendBtn.click();
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(sendDataMock).toHaveBeenCalled();
    // 错误时不应再调 onSent（数据没发出去）
    expect(onSentMock).not.toHaveBeenCalled();
    // 错误显示在表单下方
    const errDiv = container.querySelector("[data-error]");
    expect(errDiv?.textContent).toContain("串口未打开");
    expect(errDiv?.textContent).toContain("AT+RST");
  });
});

describe("PresetPanel - 添加表单", () => {
  it("添加：写入 store + 清空表单", () => {
    render();
    const contentTa = container.querySelector("textarea") as HTMLTextAreaElement;
    act(() => {
      setNativeValue(contentTa, "AT+RST");
    });
    const addBtn = getButtonByText("+ 添加");
    expect(addBtn).toBeTruthy();
    act(() => {
      addBtn!.click();
    });
    const cmds = usePresetStore.getState().commands;
    expect(cmds).toHaveLength(1);
    expect(cmds[0].content).toBe("AT+RST");
    expect(cmds[0].type).toBe("text");
    // 表单清空
    const ta = container.querySelector("textarea") as HTMLTextAreaElement;
    expect(ta.value).toBe("");
  });

  it("HEX 校验失败：显示错误，不写入", () => {
    render();
    // 切到 hex
    const typeSelect = container.querySelector("select") as HTMLSelectElement;
    act(() => {
      setNativeValue(typeSelect, "hex");
      typeSelect.dispatchEvent(new Event("change", { bubbles: true }));
    });
    const contentTa = container.querySelector("textarea") as HTMLTextAreaElement;
    act(() => {
      setNativeValue(contentTa, "ZZ"); // 非法 HEX
    });
    const addBtn = getButtonByText("+ 添加");
    act(() => {
      addBtn!.click();
    });
    expect(usePresetStore.getState().commands).toHaveLength(0);
    const errDiv = container.querySelector("[data-error]");
    expect(errDiv?.textContent).toMatch(/HEX/);
  });

  it("空内容：显示错误", () => {
    render();
    const addBtn = getButtonByText("+ 添加");
    act(() => {
      addBtn!.click();
    });
    expect(usePresetStore.getState().commands).toHaveLength(0);
    const errDiv = container.querySelector("[data-error]");
    expect(errDiv?.textContent).toContain("请输入内容");
  });
});

describe("PresetPanel - 删除", () => {
  it("点击 ✕ 调 deleteCommand", () => {
    act(() => {
      usePresetStore.setState({
        commands: [
          {
            id: "to-delete",
            content: "x",
            type: "text",
            priority: 50,
            enabled: true,
            intervalMs: 1000,
          } as PresetCommand,
        ],
      });
    });
    render();
    const deleteBtn = container.querySelector("[data-preset-row] button:last-child") as HTMLButtonElement;
    expect(deleteBtn.textContent?.trim()).toBe("✕");
    act(() => {
      deleteBtn.click();
    });
    expect(usePresetStore.getState().commands).toHaveLength(0);
  });
});
