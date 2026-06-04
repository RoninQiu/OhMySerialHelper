/**
 * 日志行解析（前端 LogPanel 用）
 *
 * 与 Rust 端 log_init::parse_line 保持一致：
 * 格式 `[YYYY-MM-DD HH:MM:SS.mmm] [LEVEL] [target] message`
 */

export type LogLevel = "DEBUG" | "INFO" | "WARN" | "ERROR";

export interface LogLine {
  /** "HH:MM:SS.mmm" */
  timestamp: string;
  /** "YYYY-MM-DD HH:MM:SS.mmm" */
  fullTimestamp: string;
  level: LogLevel;
  target: string;
  message: string;
}

/**
 * 解析一行 fern 格式日志
 *
 * @returns 解析成功返 LogLine，失败返 null（让调用方决定跳过或整行塞 message）
 */
export function parseLogLine(raw: string): LogLine | null {
  // 必须以 '[' 开头
  if (!raw.startsWith("[")) return null;
  const rest = raw.slice(1);
  // timestamp
  const endTs = rest.indexOf("]");
  if (endTs < 0) return null;
  const tsPart = rest.slice(0, endTs).trim();
  let rest2 = rest.slice(endTs + 1).trimStart();
  // level
  if (!rest2.startsWith("[")) return null;
  rest2 = rest2.slice(1);
  const endLv = rest2.indexOf("]");
  if (endLv < 0) return null;
  const levelPart = rest2.slice(0, endLv).trim();
  if (!isLogLevel(levelPart)) return null;
  let rest3 = rest2.slice(endLv + 1).trimStart();
  // target
  if (!rest3.startsWith("[")) return null;
  rest3 = rest3.slice(1);
  const endTg = rest3.indexOf("]");
  if (endTg < 0) return null;
  const targetPart = rest3.slice(0, endTg).trim();
  const message = rest3.slice(endTg + 1).trimStart();
  if (!tsPart || !levelPart || !targetPart) return null;

  // timestamp 拆日期 + HH:MM:SS.mmm
  const spaceIdx = tsPart.indexOf(" ");
  const fullTimestamp = tsPart;
  const timestamp = spaceIdx >= 0 ? tsPart.slice(spaceIdx + 1) : "";

  return {
    timestamp,
    fullTimestamp,
    level: levelPart,
    target: targetPart,
    message,
  };
}

function isLogLevel(s: string): s is LogLevel {
  return s === "DEBUG" || s === "INFO" || s === "WARN" || s === "ERROR";
}

const LEVEL_ORDER: LogLevel[] = ["DEBUG", "INFO", "WARN", "ERROR"];

/**
 * actual 级别是否 >= min 级别（不区分大小写）
 */
export function levelAtLeast(actual: string, min: string): boolean {
  const a = LEVEL_ORDER.findIndex((l) => l.toUpperCase() === actual.toUpperCase());
  const m = LEVEL_ORDER.findIndex((l) => l.toUpperCase() === min.toUpperCase());
  if (a < 0 || m < 0) return false;
  return a >= m;
}
