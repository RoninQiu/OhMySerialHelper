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

export type ViewMode = "text" | "hex";
export type Encoding = "utf8" | "gbk";
export type Direction = "rx" | "tx";

// 占位实现：commit 3 实现
export function formatLine(
  _data: Uint8Array,
  _direction: Direction,
  _viewMode: ViewMode,
  _encoding: Encoding,
  _ts: Date = new Date(),
): string {
  throw new Error("not implemented (commit 1 stub)");
}
