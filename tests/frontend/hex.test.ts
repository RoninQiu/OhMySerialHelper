/**
 * @vitest-environment jsdom
 */
import { describe, it, expect } from "vitest";
import {
  hexToBytes,
  bytesToHex,
  formatHexDump,
  isValidHex,
  crc16Modbus,
} from "../../src/utils/hex";

describe("hexToBytes", () => {
  it("parses spaced hex", () => {
    expect(hexToBytes("31 32 33")).toEqual(new Uint8Array([0x31, 0x32, 0x33]));
  });

  it("parses compact hex", () => {
    expect(hexToBytes("313233")).toEqual(new Uint8Array([0x31, 0x32, 0x33]));
  });

  it("handles empty string", () => {
    expect(hexToBytes("")).toEqual(new Uint8Array(0));
  });

  it("handles odd length with padding", () => {
    expect(hexToBytes("312")).toEqual(new Uint8Array([0x03, 0x12]));
  });
});

describe("bytesToHex", () => {
  it("converts bytes to spaced hex", () => {
    expect(bytesToHex(new Uint8Array([0x31, 0x32, 0x33]))).toBe("31 32 33");
  });

  it("handles empty array", () => {
    expect(bytesToHex(new Uint8Array(0))).toBe("");
  });

  it("pads single digit hex", () => {
    expect(bytesToHex(new Uint8Array([0x1, 0x2, 0xa]))).toBe("01 02 0a");
  });
});

describe("formatHexDump", () => {
  it("formats hex dump correctly", () => {
    const input = new Uint8Array([0x48, 0x65, 0x6c, 0x6c, 0x6f]);
    const result = formatHexDump(input);
    expect(result).toContain("48 65 6C 6C 6F");
    expect(result).toContain("Hello");
  });

  it("handles multi-line output", () => {
    const input = new Uint8Array(20);
    const result = formatHexDump(input);
    const lines = result.split("\n");
    expect(lines.length).toBe(2); // 16 + 4 bytes
  });
});

describe("isValidHex", () => {
  it("validates correct hex", () => {
    expect(isValidHex("313233")).toBe(true);
    expect(isValidHex("31 32 33")).toBe(true);
  });

  it("rejects invalid hex", () => {
    expect(isValidHex("GHIJK")).toBe(false);
    expect(isValidHex("312")).toBe(false); // odd length
  });
});

describe("crc16Modbus", () => {
  it("calculates CRC correctly", () => {
    // Modbus CRC-16 of [0x01, 0x03] is 0x2140
    const data = new Uint8Array([0x01, 0x03]);
    expect(crc16Modbus(data)).toBe(0x2140);
  });
});
