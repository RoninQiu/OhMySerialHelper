/**
 * GBK 编码转换工具
 * 注意：完整 GBK 映射表非常大，这里提供常用字符的子集
 */

// GBK 双字节范围：高位 0x81-0xFE，低位 0x40-0xFE
const GBK_HIGH_START = 0x81;
const GBK_HIGH_END = 0xFE;
const GBK_LOW_START = 0x40;
const GBK_LOW_END = 0xFE;

// 常用 GBK 字符映射表（部分）
// 格式：[gbk_high_byte, gbk_low_byte, utf8_byte1, utf8_byte2, utf8_byte3]
const GBK_MAPPING: [number, number, number, number, number][] = [
  // 中文标点
  [0xA1, 0xA1, 0xE2, 0x96, 0xA1], // ・ (中点)
  [0xA1, 0xA2, 0xE2, 0x96, 0x97], // 。 (句号)
  [0xA1, 0xA3, 0xE3, 0x80, 0x81], // 、 (顿号)
  [0xA1, 0xA4, 0xEF, 0xBC, 0x8C], // ， (逗号)
  [0xA1, 0xA5, 0xEF, 0xBC, 0x8E], // ； (分号)
  [0xA1, 0xA6, 0xEF, 0xBC, 0x9A], // ： (冒号)
  [0xA1, 0xA7, 0xEF, 0xBC, 0x9F], // ？ (问号)
  [0xA1, 0xA8, 0xEF, 0xBC, 0x81], // ！ (感叹号)
  [0xA1, 0xA9, 0xE2, 0x80, 0xA6], // …… (省略号)
  [0xA1, 0xAA, 0xE2, 0x80, 0x94], // —— (破折号)
  [0xA1, 0xAB, 0xE3, 0x80, 0x8A], // 「 (左引号)
  [0xA1, 0xAC, 0xE3, 0x80, 0x8B], // 」 (右引号)
  [0xA1, 0xAD, 0xE3, 0x80, 0x8C], // 『 (左双引号)
  [0xA1, 0xAE, 0xE3, 0x80, 0x8D], // 』 (右双引号)
  [0xA1, 0xAF, 0xE3, 0x80, 0x90], // （ (左括号)
  [0xA1, 0xB0, 0xE3, 0x80, 0x91], // ） (右括号)
  [0xA1, 0xB1, 0xE3, 0x80, 0x94], // 【 (左书名号)
  [0xA1, 0xB2, 0xE3, 0x80, 0x95], // 】 (右书名号)
  [0xA1, 0xB3, 0xE2, 0x96, 0xB6], // △ (三角形)
  [0xA1, 0xB4, 0xE2, 0x97, 0x87], // ☆ (空心星)
  [0xA1, 0xB5, 0xE2, 0x97, 0x8F], // ● (实心圆)
  [0xA1, 0xB6, 0xE2, 0x97, 0x8A], // ○ (空心圆)
  [0xA1, 0xB7, 0xE2, 0x96, 0xBC], // ■ (实心方块)
  [0xA1, 0xB8, 0xE2, 0x96, 0xA0], // □ (空心方块)
  [0xA1, 0xB9, 0xE2, 0x98, 0xBE], // ▼ (向下三角)
  [0xA1, 0xBA, 0xE2, 0x98, 0xBB], // ▲ (向上三角)

  // GBK 第一区（ASCII 兼容区）
  [0xA8, 0xBC, 0xE7, 0xA7, 0x81], // 龟
  [0xA8, 0xBD, 0xE9, 0x9B, 0x80], // 龜

  // 数字和符号
  [0xA2, 0xE1, 0xE2, 0x85, 0x96], // ①
  [0xA2, 0xE2, 0xE2, 0x85, 0x97], // ②
  [0xA2, 0xE3, 0xE2, 0x85, 0x98], // ③
];

// 创建 GBK 到 UTF-8 的快速查找表
const GBK_TO_UTF8_MAP = new Map<string, Uint8Array>();
for (const [hi, lo, ...utf8] of GBK_MAPPING) {
  const key = ((hi << 8) | lo).toString(16).toUpperCase();
  GBK_TO_UTF8_MAP.set(key, new Uint8Array(utf8));
}

/**
 * GBK 字节数组转换为 UTF-8 字符串
 */
export function decodeGBK(buffer: Uint8Array): string {
  const chunks: string[] = [];
  let i = 0;

  while (i < buffer.length) {
    const byte = buffer[i];

    if (byte < 0x80) {
      // ASCII
      chunks.push(String.fromCharCode(byte));
      i++;
    } else if (byte >= GBK_HIGH_START && byte <= GBK_HIGH_END && i + 1 < buffer.length) {
      // GBK 双字节
      const lo = buffer[i + 1];
      if (lo >= GBK_LOW_START && lo <= GBK_LOW_END) {
        const gbkCode = ((byte << 8) | lo).toString(16).toUpperCase();
        const utf8Bytes = GBK_TO_UTF8_MAP.get(gbkCode);
        if (utf8Bytes) {
          const decoder = new TextDecoder("utf-8", { fatal: false });
          chunks.push(decoder.decode(utf8Bytes));
        } else {
          // 未知 GBK 字符，用替换符
          chunks.push("\uFFFD");
        }
        i += 2;
      } else {
        // 低位不符合 GBK 规范，当作单字节处理
        chunks.push(String.fromCharCode(byte));
        i++;
      }
    } else {
      // 非 GBK 高位，当作单字节处理
      chunks.push(String.fromCharCode(byte));
      i++;
    }
  }

  return chunks.join("");
}

/**
 * 检测字节数组的编码
 * 策略：检查 UTF-8 解码是否成功，如果出现替换符则可能是 GBK
 */
export function detectEncoding(buffer: Uint8Array): "utf8" | "gbk" | "latin1" {
  if (buffer.length === 0) {
    return "utf8";
  }

  // 检查是否包含高位字节
  const hasHighBytes = buffer.some(b => b >= 0x80);
  if (!hasHighBytes) {
    return "latin1"; // 纯 ASCII
  }

  // 尝试 UTF-8 解码
  try {
    const decoded = new TextDecoder("utf-8", { fatal: true }).decode(buffer);
    return "utf8";
  } catch {
    // UTF-8 解码失败，可能是 GBK 或其他编码
    return "gbk";
  }
}

/**
 * 将字符串转换为 GBK 编码的字节数组
 * 注意：只处理 ASCII 和已知 GBK 字符
 */
export function encodeGBK(text: string): Uint8Array {
  const chunks: number[] = [];

  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);

    if (code < 0x80) {
      // ASCII
      chunks.push(code);
    } else {
      // 查找 GBK 映射表的反向映射
      let found = false;
      for (const [hi, lo, ...utf8] of GBK_MAPPING) {
        const decoder = new TextDecoder("utf-8");
        const char = decoder.decode(new Uint8Array(utf8));
        if (char === text[i]) {
          chunks.push(hi, lo);
          found = true;
          break;
        }
      }
      if (!found) {
        // 无法编码，用 ? 代替
        chunks.push(0x3F);
      }
    }
  }

  return new Uint8Array(chunks);
}
