/**
 * Tauri API Mock
 *
 * 用于在 Vitest 环境下模拟 @tauri-apps/api/core 的 invoke
 * 和 @tauri-apps/api/event 的 listen
 *
 * 注意：@tauri-apps/api/core 的 Channel 由测试文件本地 vi.mock 提供
 * （需直接返回 class，不能用动态 import.then 因为源码是 `new Channel()` 同步构造）
 */

import { vi } from "vitest";

export const mockInvoke = vi.fn();
export const eventListeners = new Map<string, (event: { payload: unknown }) => void>();

/**
 * 触发 mock 事件
 * 测试中调用：emitMockEvent("serial-data", new Uint8Array([1,2,3]))
 */
export function emitMockEvent(event: string, payload: unknown) {
  const listener = eventListeners.get(event);
  if (listener) listener({ payload });
}

export function clearMockEvents() {
  eventListeners.clear();
}

export const mockTauriApi = {
  invoke: mockInvoke,
  listen: vi.fn(async (event: string, callback: (e: { payload: unknown }) => void) => {
    eventListeners.set(event, callback);
    return () => eventListeners.delete(event);
  }),
};
