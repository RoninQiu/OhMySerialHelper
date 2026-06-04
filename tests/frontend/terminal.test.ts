/**
 * @vitest-environment jsdom
 *
 * Terminal 纯函数单测
 * - formatTimestamp：HH:MM:SS.mmm 格式
 * - byteHex：两字符大写
 */
import { describe, it, expect } from "vitest";
import { formatTimestamp, byteHex } from "../../src/components/Terminal";

describe("formatTimestamp", () => {
  it("标准时刻格式正确", () => {
    const d = new Date(2026, 0, 1, 13, 5, 9, 7);
    // 注意 month=0 = 一月
    expect(formatTimestamp(d)).toBe("13:05:09.007");
  });

  it("个位数时/分/秒补零", () => {
    const d = new Date(2026, 0, 1, 1, 2, 3, 4);
    expect(formatTimestamp(d)).toBe("01:02:03.004");
  });

  it("毫秒不满三位补零（5 → 005）", () => {
    const d = new Date(2026, 0, 1, 0, 0, 0, 5);
    expect(formatTimestamp(d)).toBe("00:00:00.005");
  });

  it("午夜 00:00:00.000", () => {
    const d = new Date(2026, 0, 1, 0, 0, 0, 0);
    expect(formatTimestamp(d)).toBe("00:00:00.000");
  });

  it("长度固定 12 字符", () => {
    const d = new Date();
    expect(formatTimestamp(d).length).toBe(12);
  });
});

describe("byteHex", () => {
  it("0 → 00", () => {
    expect(byteHex(0)).toBe("00");
  });

  it("0xFF → FF", () => {
    expect(byteHex(0xff)).toBe("FF");
  });

  it("0x0a → 0A", () => {
    expect(byteHex(0x0a)).toBe("0A");
  });

  it("始终两字符大写", () => {
    for (let b = 0; b < 256; b++) {
      const s = byteHex(b);
      expect(s).toMatch(/^[0-9A-F]{2}$/);
    }
  });
});
