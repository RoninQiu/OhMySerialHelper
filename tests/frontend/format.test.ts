import { describe, it, expect } from "vitest";
import { bytesToHuman } from "../../src/utils/format";

describe("bytesToHuman", () => {
  it("0 字节", () => {
    expect(bytesToHuman(0)).toBe("0 B");
  });

  it("小于 1024 字节显示原值", () => {
    expect(bytesToHuman(512)).toBe("512 B");
    expect(bytesToHuman(1023)).toBe("1023 B");
  });

  it("正好 1024 字节 = 1 KB", () => {
    expect(bytesToHuman(1024)).toBe("1.00 KB");
  });

  it("8192 字节 = 8 KB", () => {
    expect(bytesToHuman(8192)).toBe("8.00 KB");
  });

  it("1234567 字节 ≈ 1.18 MB", () => {
    expect(bytesToHuman(1234567)).toBe("1.18 MB");
  });

  it("1024 * 1024 = 1 MB", () => {
    expect(bytesToHuman(1024 * 1024)).toBe("1.00 MB");
  });

  it("负数 / NaN 兜底", () => {
    expect(bytesToHuman(-1)).toBe("0 B");
    expect(bytesToHuman(NaN)).toBe("0 B");
  });
});
