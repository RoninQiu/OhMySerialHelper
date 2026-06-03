/**
 * 格式化工具
 */

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/**
 * 把字节数格式化为人类可读字符串
 *
 * @example
 * bytesToHuman(0)        // "0 B"
 * bytesToHuman(1023)     // "1023 B"
 * bytesToHuman(1024)     // "1.00 KB"
 * bytesToHuman(1234567)  // "1.18 MB"
 * bytesToHuman(8192)     // "8.00 KB"
 */
export function bytesToHuman(bytes: number, decimals = 2): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "0 B";
  }

  let value = bytes;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < UNITS.length - 1) {
    value /= 1024;
    unitIdx++;
  }

  if (unitIdx === 0) {
    return `${value} ${UNITS[unitIdx]}`;
  }
  return `${value.toFixed(decimals)} ${UNITS[unitIdx]}`;
}
