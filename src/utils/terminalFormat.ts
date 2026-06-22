/**
 * 终端格式化工具（v1.2.0）
 *
 * 用途：
 * 1. formatLine - 生成去 ANSI 的纯文本行（用于录制文件）
 * 2. 从 Terminal.tsx 提取 formatTimestamp / byteHex（保持 single source of truth）
 *
 * 与 Terminal.tsx 现有 ANSI 着色逻辑共享底层函数，但 formatLine
 * 输出纯文本（无 \x1b[...] 序列），便于 grep 和文本分析。
 */

import { decodeGBK } from "./encoding";

export type ViewMode = "text" | "hex";
export type Encoding = "utf8" | "gbk";
export type Direction = "rx" | "tx";

/** 格式化时间戳：HH:MM:SS.mmm */
export function formatTimestamp(d: Date): string {
  const pad2 = (n: number) => n.toString().padStart(2, "0");
  const pad3 = (n: number) => n.toString().padStart(3, "0");
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}.${pad3(
    d.getMilliseconds(),
  )}`;
}

/** 单字节 → 两字符大写 hex */
export function byteHex(b: number): string {
  return b.toString(16).padStart(2, "0").toUpperCase();
}

/**
 * 生成去 ANSI 的纯文本行（不含 \n），由前端 Terminal + App 系统消息复用
 *
 * 格式：
 * - HEX 视图：`[HH:MM:SS.mmm] ← [HEX] AA BB CC DD`
 * - TEXT 视图：`[HH:MM:SS.mmm] ← hello world`（无 [HEX] 标签，便于 grep）
 */
export function formatLine(
  data: Uint8Array,
  direction: Direction,
  viewMode: ViewMode,
  encoding: Encoding,
  ts: Date = new Date(),
): string {
  const tsStr = formatTimestamp(ts);
  const arrow = direction === "rx" ? "←" : "→";

  if (viewMode === "hex") {
    const hex = Array.from(data, byteHex).join(" ");
    return `[${tsStr}] ${arrow} [HEX] ${hex}`;
  }

  // 文本视图：按编码解码
  const text =
    encoding === "gbk"
      ? decodeGBK(data)
      : new TextDecoder("utf-8").decode(data);
  return `[${tsStr}] ${arrow} ${text}`;
}