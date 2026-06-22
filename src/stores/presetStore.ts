import { create } from "zustand";
import { persist } from "zustand/middleware";
import { hexToBytes } from "../utils/hex";

/**
 * 预设命令：v3 简化（去掉 name 字段）
 * - 用户记的是字节序列本身（AA 55 03 01），不是抽象标签
 * - 列表直接展示 content 预览，无需 name 中转
 * - content 是 single source of truth
 */
export interface PresetCommand {
  id: string;
  content: string;
  type: "text" | "hex";
  priority: number; // 1-100
  enabled: boolean;
  intervalMs: number;
}

interface PresetState {
  commands: PresetCommand[];
  isPolling: boolean;

  addCommand: (cmd: Omit<PresetCommand, "id">) => Promise<void>;
  updateCommand: (id: string, updates: Partial<PresetCommand>) => void;
  deleteCommand: (id: string) => Promise<void>;
  startPolling: () => Promise<void>;
  stopPolling: () => Promise<void>;
}

/**
 * 持久化迁移函数（导出以便测试）
 * v1 → v3 任意旧版本：丢弃 isPolling 字段（避免上次的轮询状态残留）
 * v3 起：旧版本的 name 字段不再使用，但不在 migrate 中显式 strip（避免遍历大数组）；
 *        UI/接口层不读 name，残留无害
 */
export function migratePreset(
  persistedState: unknown,
  _version: number,
): PresetState {
  const state = persistedState as { isPolling?: boolean } & Record<string, unknown>;
  if (state && typeof state === "object" && "isPolling" in state) {
    delete state.isPolling;
  }
  return state as unknown as PresetState;
}

/** 把文本/HEX 字符串转 Vec<u8>，给 Rust 用 */
function contentToBytes(cmd: PresetCommand): Uint8Array {
  if (cmd.type === "text") {
    return new TextEncoder().encode(cmd.content);
  }
  return hexToBytes(cmd.content);
}

export const usePresetStore = create<PresetState>()(
  persist(
    (set, get) => ({
      commands: [],
      isPolling: false,

      addCommand: async (cmd) => {
        const id = crypto.randomUUID();
        set((s) => ({
          commands: [...s.commands, { ...cmd, id }],
        }));
        // 同步到 Rust 队列
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("cmd_queue_add", {
            id,
            content: Array.from(contentToBytes({ ...cmd, id })),
            priority: cmd.priority,
            intervalMs: cmd.intervalMs,
          });
        } catch (e) {
          console.error("Failed to add to Rust queue:", e);
        }
      },

      updateCommand: (id, updates) =>
        set((s) => ({
          commands: s.commands.map((c) =>
            c.id === id ? { ...c, ...updates } : c,
          ),
        })),

      deleteCommand: async (id) => {
        set((s) => ({
          commands: s.commands.filter((c) => c.id !== id),
        }));
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("cmd_queue_remove", { id });
        } catch (e) {
          console.error("Failed to remove from Rust queue:", e);
        }
      },

      startPolling: async () => {
        // 重新同步当前所有命令到 Rust 队列
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("cmd_queue_clear");
          for (const cmd of get().commands.filter((c) => c.enabled)) {
            await invoke("cmd_queue_add", {
              id: cmd.id,
              content: Array.from(contentToBytes(cmd)),
              priority: cmd.priority,
              intervalMs: cmd.intervalMs,
            });
          }
          await invoke("cmd_queue_start_polling");
          set({ isPolling: true });
        } catch (e) {
          console.error("Failed to start polling:", e);
          throw e;
        }
      },

      stopPolling: async () => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("cmd_queue_stop_polling");
        } catch (e) {
          console.error("Failed to stop polling:", e);
        }
        set({ isPolling: false });
      },
    }),
    {
      name: "presets",
      version: 3,
      migrate: migratePreset,
    },
  ),
);
