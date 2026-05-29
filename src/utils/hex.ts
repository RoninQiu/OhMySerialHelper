/**
 * HEX 转换和格式化工具
 */

/**
 * 将十六进制字符串转换为字节数组
 * 支持格式：带空格 "31 32 33" 和不带空格 "313233"
 */
export function hexToBytes(hex: string): Uint8Array {
  // 清理十六进制字符串：移除空格、支持 313233 或 31 32 33 格式
  const cleaned = hex.replace(/\s+/g, "");

  // 验证输入
  if (!/^[0-9A-Fa-f]*$/.test(cleaned)) {
    throw new Error("无效的十六进制字符");
  }

  // 奇数长度时补零
  const padded = cleaned.length % 2 === 0 ? cleaned : "0" + cleaned;
  const bytes = new Uint8Array(padded.length / 2);

  for (let i = 0; i < padded.length; i += 2) {
    bytes[i / 2] = parseInt(padded.substr(i, 2), 16);
  }
  return bytes;
}

/**
 * 将字节数组转换为带空格的十六进制字符串
 */
export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join(" ");
}

/**
 * 格式化十六进制转储（类似 010editor 或 Wireshark 风格）
 */
export function formatHexDump(
  bytes: Uint8Array,
  bytesPerLine: number = 16,
  startOffset: number = 0
): string {
  const lines: string[] = [];

  for (let i = 0; i < bytes.length; i += bytesPerLine) {
    const chunk = bytes.slice(i, i + bytesPerLine);
    const addr = (startOffset + i).toString(16).padStart(8, "0").toUpperCase();

    // HEX 部分
    const hexParts: string[] = [];
    for (let j = 0; j < bytesPerLine; j++) {
      if (j < chunk.length) {
        hexParts.push(chunk[j].toString(16).padStart(2, "0").toUpperCase());
      } else {
        hexParts.push("  ");
      }
    }

    // 分成两半，每半8字节
    const hexLeft = hexParts.slice(0, 8).join(" ");
    const hexRight = hexParts.slice(8, 16).join(" ");

    // ASCII 部分
    const ascii = Array.from(chunk)
      .map((b) => (b >= 0x20 && b <= 0x7e) ? String.fromCharCode(b) : ".")
      .join("");

    lines.push(`${addr}  ${hexLeft}  ${hexRight}  ${ascii}`);
  }

  return lines.join("\n");
}

/**
 * 从 HEX 转储中提取可打印字符
 */
export function extractPrintableAscii(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => (b >= 0x20 && b <= 0x7e) ? String.fromCharCode(b) : ".")
    .join("");
}

/**
 * 计算校验和（CRC-16/MODBUS）
 */
export function crc16Modbus(data: Uint8Array): number {
  let crc = 0xFFFF;

  for (const byte of data) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) {
      if (crc & 0x0001) {
        crc = (crc >> 1) ^ 0xA001;
      } else {
        crc >>= 1;
      }
    }
  }

  return crc;
}

/**
 * 验证 HEX 字符串格式
 */
export function isValidHex(hex: string): boolean {
  const cleaned = hex.replace(/\s+/g, "");
  return /^[0-9A-Fa-f]+$/.test(cleaned) && cleaned.length % 2 === 0;
}
