/**
 * @vitest-environment jsdom
 */
import { describe, it, expect } from "vitest";
import { matchHotkey, Hotkey } from "../../src/hooks/useHotkeys";

function makeEvent(opts: Partial<KeyboardEventInit> = {}): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    key: "a",
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    ...opts,
  });
}

const hk = (overrides: Partial<Hotkey> = {}): Hotkey => ({
  key: "a",
  handler: () => {},
  ...overrides,
});

describe("matchHotkey", () => {
  it("基础按键匹配", () => {
    const ev = makeEvent({ key: "a" });
    expect(matchHotkey({ hotkey: hk(), event: ev, target: null })).toBe(true);
  });

  it("Ctrl+Enter 匹配", () => {
    const ev = makeEvent({ key: "Enter", ctrlKey: true });
    expect(
      matchHotkey({ hotkey: hk({ key: "Enter", ctrl: true }), event: ev, target: null }),
    ).toBe(true);
  });

  it("不带 Ctrl 不应触发 Ctrl 修饰键的 hotkey", () => {
    const ev = makeEvent({ key: "Enter" });
    expect(
      matchHotkey({ hotkey: hk({ key: "Enter", ctrl: true }), event: ev, target: null }),
    ).toBe(false);
  });

  it("大小写不敏感", () => {
    const ev = makeEvent({ key: "l", ctrlKey: true });
    expect(
      matchHotkey({ hotkey: hk({ key: "L", ctrl: true }), event: ev, target: null }),
    ).toBe(true);
  });

  it("input 中默认不触发", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    const ev = makeEvent({ key: "a" });
    expect(matchHotkey({ hotkey: hk(), event: ev, target: input })).toBe(false);
    document.body.removeChild(input);
  });

  it("inEditable=true 时 input 中也触发", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    const ev = makeEvent({ key: "Enter", ctrlKey: true });
    expect(
      matchHotkey({
        hotkey: hk({ key: "Enter", ctrl: true, inEditable: true }),
        event: ev,
        target: input,
      }),
    ).toBe(true);
    document.body.removeChild(input);
  });

  it("textarea 中默认不触发", () => {
    const ta = document.createElement("textarea");
    document.body.appendChild(ta);
    const ev = makeEvent({ key: "a" });
    expect(matchHotkey({ hotkey: hk(), event: ev, target: ta })).toBe(false);
    document.body.removeChild(ta);
  });

  it("修饰键不匹配时不触发", () => {
    const ev = makeEvent({ key: "l", shiftKey: true });
    expect(
      matchHotkey({ hotkey: hk({ key: "l", ctrl: true }), event: ev, target: null }),
    ).toBe(false);
  });
});
