/**
 * 字体/字号相关常量与纯函数
 * 与 Rust 端 AppConfig.font_size / AppConfig.font_family 一一对应
 */

/** xterm 默认字体栈（跨平台兜底） */
export const SYSTEM_DEFAULT_FAMILY =
  "Consolas, Monaco, 'Courier New', monospace";

/** 字号保留字（与 Rust 端 default_font_family() 一致） */
export const SYSTEM_DEFAULT_KEY = "system-default";

/** 字号范围与默认值 */
export const FONT_SIZE_RANGE = {
  min: 12,
  max: 24,
  step: 2,
  default: 14,
} as const;

/** 字号档位标签（UI 显示用） */
export const FONT_SIZE_LABELS: Record<number, string> = {
  12: "小",
  14: "标准",
  16: "偏大",
  18: "大",
  20: "很大",
  22: "超大",
  24: "特大",
};

/** clamp 字号到合法范围 */
export function clampFontSize(n: number): number {
  return Math.max(FONT_SIZE_RANGE.min, Math.min(FONT_SIZE_RANGE.max, n));
}

/**
 * 解析 store 里的字体字段为 xterm 可用的 fontFamily 字符串
 * - "system-default" / "" / null → fallback 常量
 * - 其他 → "<name>, <fallback>"
 */
export function resolveFontFamily(name: string | null | undefined): string {
  if (
    !name ||
    name === SYSTEM_DEFAULT_KEY ||
    name.trim() === ""
  ) {
    return SYSTEM_DEFAULT_FAMILY;
  }
  return `${name}, ${SYSTEM_DEFAULT_FAMILY}`;
}