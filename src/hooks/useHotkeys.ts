/**
 * 全局快捷键 hook
 *
 * 用法：
 *   useHotkeys([
 *     { key: "Enter", ctrl: true, handler: send },
 *     { key: "l", ctrl: true, handler: clear },
 *   ]);
 */
import { useEffect } from "react";

export interface Hotkey {
  /** 按键名（不区分大小写） */
  key: string;
  /** 是否需要 Ctrl 修饰键（默认 false） */
  ctrl?: boolean;
  /** 是否需要 Shift 修饰键（默认 false） */
  shift?: boolean;
  /** 是否需要 Alt 修饰键（默认 false） */
  alt?: boolean;
  /** 触发回调 */
  handler: (e: KeyboardEvent) => void;
  /** 是否在 input/textarea 中也触发（默认 false，避免冲突） */
  inEditable?: boolean;
}

export interface HotkeyMatchInput {
  hotkey: Hotkey;
  event: KeyboardEvent;
  target: HTMLElement | null;
}

/** 判断单个 hotkey 是否匹配事件（纯函数，可单测） */
export function matchHotkey({
  hotkey,
  event,
  target,
}: HotkeyMatchInput): boolean {
  // 修饰键匹配
  if (!!hotkey.ctrl !== event.ctrlKey) return false;
  if (!!hotkey.shift !== event.shiftKey) return false;
  if (!!hotkey.alt !== event.altKey) return false;

  // 按键匹配
  if (event.key.toLowerCase() !== hotkey.key.toLowerCase()) return false;

  // 可编辑元素过滤
  if (target) {
    const isEditable =
      target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.isContentEditable;
    if (isEditable && !hotkey.inEditable) return false;
  }

  return true;
}

export function useHotkeys(hotkeys: Hotkey[]): void {
  useEffect(() => {
    const listener = (e: KeyboardEvent) => {
      for (const hk of hotkeys) {
        if (matchHotkey({ hotkey: hk, event: e, target: e.target as HTMLElement | null })) {
          e.preventDefault();
          hk.handler(e);
          return;
        }
      }
    };

    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [hotkeys]);
}
