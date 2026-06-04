/**
 * 日志轮询 hook
 *
 * 每 intervalMs 调 cmd_read_log_lines 拉最新日志，注入 logStore
 * 用 setInterval + 清理函数，组件卸载时自动停止
 *
 * 设计：
 * - 默认 intervalMs = 2000（不抢 60Hz 串口热路径，符合"不违背高效收发"）
 * - enabled=false 时暂停
 * - 拉取失败静默（用 console.warn），不打扰用户
 */
import { useEffect, useRef } from "react";
import { useLogStore } from "../stores/logStore";
import type { LogLine } from "../utils/logParser";

export interface UseLogPollingOptions {
  /** 拉取间隔（毫秒），默认 2000 */
  intervalMs?: number;
  /** 每次拉多少行，默认 200 */
  limit?: number;
  /** 是否启用，默认 true */
  enabled?: boolean;
}

const DEFAULT_INTERVAL_MS = 2000;
const DEFAULT_LIMIT = 200;

export function useLogPolling(options: UseLogPollingOptions = {}): void {
  const {
    intervalMs = DEFAULT_INTERVAL_MS,
    limit = DEFAULT_LIMIT,
    enabled = true,
  } = options;

  // 保留最新 options（intervalMs/limit 变时 ref 跟上，避免重启 effect）
  const optsRef = useRef({ intervalMs, limit, enabled });
  optsRef.current = { intervalMs, limit, enabled };

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let timer: number | null = null;

    const fetchOnce = async () => {
      const { limit: l } = optsRef.current;
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const lines = await invoke<LogLine[]>("cmd_read_log_lines", {
          offset: 0,
          limit: l,
          levelFilter: null,
        });
        if (cancelled) return;
        useLogStore.getState().setLines(lines);
      } catch (e) {
        // 静默：开发期会失败（非 Tauri 环境）
        if (!cancelled) {
          console.warn("useLogPolling: 拉取失败", e);
        }
      }
    };

    // 立即拉一次，然后定时
    void fetchOnce();
    timer = window.setInterval(fetchOnce, optsRef.current.intervalMs);

    return () => {
      cancelled = true;
      if (timer !== null) clearInterval(timer);
    };
  }, [enabled, intervalMs]);
}
