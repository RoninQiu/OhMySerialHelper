/**
 * 日志状态（前端 LogPanel 用）
 *
 * 缓存最近拉取的日志行 + 用户过滤偏好（level 最小值 / 关键字）
 * 不持久化：重启应用重新拉取
 */
import { create } from "zustand";
import type { LogLine, LogLevel } from "../utils/logParser";

export type LevelFilter = "DEBUG" | "INFO" | "WARN" | "ERROR" | "ALL";

interface LogState {
  /** 原始日志行（按时间正序） */
  lines: LogLine[];
  /** 最小显示级别（ALL = 不过滤） */
  levelFilter: LevelFilter;
  /** 关键字过滤（空字符串 = 不过滤） */
  keyword: string;
  /** 上次刷新时间戳（毫秒） */
  lastFetchedAt: number | null;
  /** 是否正在拉取 */
  loading: boolean;

  setLines: (lines: LogLine[]) => void;
  setLevelFilter: (level: LevelFilter) => void;
  setKeyword: (keyword: string) => void;
  setLoading: (loading: boolean) => void;
  clear: () => void;
}

/**
 * 应用过滤：level + keyword
 * 纯函数：把过滤逻辑抽出来便于测试 + 复用
 */
export function applyFilter(
  lines: LogLine[],
  levelFilter: LevelFilter,
  keyword: string,
): LogLine[] {
  const k = keyword.trim().toLowerCase();
  return lines.filter((l) => {
    // level
    if (levelFilter !== "ALL" && !levelAtLeastIndex(l.level, levelFilter)) {
      return false;
    }
    // keyword（不区分大小写、子串匹配）
    if (k) {
      const hay = `${l.message} ${l.target}`.toLowerCase();
      if (!hay.includes(k)) return false;
    }
    return true;
  });
}

const LEVEL_ORDER: LogLevel[] = ["DEBUG", "INFO", "WARN", "ERROR"];

function levelAtLeastIndex(actual: LogLevel, min: LogLevel): boolean {
  return LEVEL_ORDER.indexOf(actual) >= LEVEL_ORDER.indexOf(min);
}

export const useLogStore = create<LogState>((set) => ({
  lines: [],
  levelFilter: "ALL",
  keyword: "",
  lastFetchedAt: null,
  loading: false,

  setLines: (lines) => set({ lines, lastFetchedAt: Date.now(), loading: false }),
  setLevelFilter: (levelFilter) => set({ levelFilter }),
  setKeyword: (keyword) => set({ keyword }),
  setLoading: (loading) => set({ loading }),
  clear: () => set({ lines: [], lastFetchedAt: null }),
}));
