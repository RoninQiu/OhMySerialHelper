/**
 * 全局应用配置
 *
 * - 启动时从 Rust 拉一次（cmd_load_config），覆盖各 store 默认值
 * - 任何相关 store 改变时，自动写回（cmd_save_config）
 * - 字段：lastPort / baudRate / dataBits / stopBits / parity / encoding /
 *   theme / bufferSize / autoReconnect / reconnectMaxAttempts
 *
 * 设计：合并而非替换
 * - Rust 配置优先（持久化在 .json）
 * - 前端 uiStore 仍保留 zustand persist 作为 fallback（无 Tauri 环境也能用）
 * - 字段同名时以 Rust 为准
 */
import { create } from "zustand";
import { useSerialStore } from "./serialStore";
import { useBufferStore, BUFFER_SIZES } from "./bufferStore";
import { useUiStore } from "./uiStore";

export interface AppConfigFE {
  last_port: string | null;
  baud_rate: number;
  data_bits: 5 | 6 | 7 | 8;
  stop_bits: 1 | 2;
  parity: "none" | "odd" | "even";
  encoding: "utf8" | "gbk";
  theme: "dark" | "light" | "system";
  buffer_size: number;
  auto_reconnect: boolean;
  reconnect_max_attempts: number;
  font_size: number;
  font_family: string;
  /// v1.2.0 录制：默认保存路径（空 = 每次弹对话框）
  default_capture_path: string;
  /// v1.2.0 录制：每次录制时弹文件对话框
  prompt_save_dialog: boolean;
}

export const DEFAULT_CONFIG: AppConfigFE = {
  last_port: null,
  baud_rate: 115200,
  data_bits: 8,
  stop_bits: 1,
  parity: "none",
  encoding: "utf8",
  theme: "dark",
  buffer_size: 65536,
  auto_reconnect: true,
  reconnect_max_attempts: 5,
  font_size: 14,
  font_family: "system-default",
  default_capture_path: "",
  prompt_save_dialog: true,
};

interface ConfigState {
  config: AppConfigFE;
  loaded: boolean;
  /** 标记正在恢复初始值，避免触发写盘 */
  loading: boolean;
  /** 从 Rust 加载并应用（启动时调用一次） */
  loadFromBackend: () => Promise<void>;
  /** 立即写盘（debounce 由 useConfigSync 处理） */
  save: () => Promise<void>;
  /** clamp 字号到 12-24，写入 config.font_size */
  setFontSize: (n: number) => void;
  /** trim 字体名；空串回退到 'system-default' */
  setFontFamily: (name: string) => void;
}

/** 检查是否在 Tauri 环境 */
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** 简化的 console.* 封装 */
const log = {
  info: (...a: unknown[]) => console.info("[config]", ...a),
  warn: (...a: unknown[]) => console.warn("[config]", ...a),
};

/** 把任意 number 安全收窄到 BufferSize 联合类型 */
function toBufferSize(n: number): (typeof BUFFER_SIZES)[number] {
  return (BUFFER_SIZES as readonly number[]).includes(n)
    ? (n as (typeof BUFFER_SIZES)[number])
    : (DEFAULT_CONFIG.buffer_size as (typeof BUFFER_SIZES)[number]);
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: DEFAULT_CONFIG,
  loaded: false,
  loading: false,

  loadFromBackend: async () => {
    if (!isTauri()) {
      set({ loaded: true });
      return;
    }
    set({ loading: true });
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const cfg = (await invoke("cmd_load_config")) as Partial<AppConfigFE>;
      const merged: AppConfigFE = { ...DEFAULT_CONFIG, ...cfg };

      // 应用到各 store
      useSerialStore.getState().setBaudRate(merged.baud_rate);
      if (merged.last_port) {
        // 仅作为"上次使用的端口"记录，不自动打开
        useSerialStore.setState({ portName: merged.last_port });
      }
      useSerialStore.getState().setEncoding(merged.encoding);
      useBufferStore.getState().setBufferSize(toBufferSize(merged.buffer_size));
      useUiStore.getState().setTheme(merged.theme);

      set({ config: merged, loaded: true, loading: false });
      log.info("config loaded from backend", merged);
    } catch (e) {
      log.warn("config load failed, using defaults:", e);
      set({ loaded: true, loading: false });
    }
  },

  save: async () => {
    if (!isTauri()) return;
    if (get().loading) return; // 启动恢复期间不写盘
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("cmd_save_config", { config: get().config });
    } catch (e) {
      log.warn("config save failed:", e);
    }
  },

  setFontSize: (n: number) => {
    const clamped = Math.max(12, Math.min(24, n));
    set((state) => ({ config: { ...state.config, font_size: clamped } }));
  },
  setFontFamily: (name: string) => {
    const trimmed = name.trim() || "system-default";
    set((state) => ({
      config: { ...state.config, font_family: trimmed },
    }));
  },
}));
