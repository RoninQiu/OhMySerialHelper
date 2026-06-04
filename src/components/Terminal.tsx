import { useEffect, useRef, useCallback, forwardRef, useImperativeHandle } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { decodeGBK } from "../utils/encoding";
import { APP_VERSION } from "../utils/version";
import { useUiStore } from "../stores/uiStore";

export type Direction = "rx" | "tx";

export interface TerminalHandle {
  writeData: (data: Uint8Array, direction?: Direction) => void;
  clear: () => void;
}

interface TerminalProps {
  viewMode: "text" | "hex";
  encoding: "utf8" | "gbk";
}

const XTERM_THEMES = {
  dark: {
    background: "#1f2937",
    foreground: "#f3f4f6",
    cursor: "#3b82f6",
    cursorAccent: "#1f2937",
    selectionBackground: "#3b82f680",
  },
  light: {
    background: "#ffffff",
    foreground: "#1f2937",
    cursor: "#2563eb",
    cursorAccent: "#ffffff",
    selectionBackground: "#3b82f640",
  },
} as const;

// ANSI 颜色（256 色前景）：RX 蓝字 / TX 绿字（v1.0.1: 改用前景色，去掉大块底色）
const ANSI_RESET = "\x1b[0m";
const RX_FG = "\x1b[38;5;111m"; // 蓝灰字（dark/light 都可读）
const TX_FG = "\x1b[38;5;71m"; // 绿灰字
const DIM = "\x1b[2m"; // 时间戳暗显
const BRIGHT = "\x1b[1m"; // 方向箭头加粗

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

export const Terminal = forwardRef<TerminalHandle, TerminalProps>(
  ({ viewMode, encoding }, ref) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<XTerm | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const resolvedTheme = useUiStore((s) => s.resolvedTheme());

    // 初始化 xterm
    useEffect(() => {
      if (!terminalRef.current) return;

      const xterm = new XTerm({
        theme: XTERM_THEMES.dark,
        cursorBlink: true,
        fontSize: 14,
        fontFamily: "Consolas, Monaco, 'Courier New', monospace",
      });

      const fitAddon = new FitAddon();
      xterm.loadAddon(fitAddon);
      xterm.open(terminalRef.current);
      fitAddon.fit();

      xtermRef.current = xterm;
      fitAddonRef.current = fitAddon;

      xterm.write(`OhMySerial v${APP_VERSION}\r\n`);
      xterm.write("=================\r\n\r\n");
      xterm.write("串口调试助手已就绪\r\n");
      xterm.write("请选择串口并点击连接...\r\n\r\n");

      return () => {
        xterm.dispose();
      };
    }, []);

    // 窗口缩放
    useEffect(() => {
      const handleResize = () => fitAddonRef.current?.fit();
      window.addEventListener("resize", handleResize);
      return () => window.removeEventListener("resize", handleResize);
    }, []);

    // 主题切换（不重建实例，xterm 支持 setOption）
    useEffect(() => {
      if (xtermRef.current) {
        xtermRef.current.options.theme = XTERM_THEMES[resolvedTheme];
      }
    }, [resolvedTheme]);

    /**
     * 写一帧数据
     * 设计：每次 emit 视为一帧，输出一行 [ts] [方向] [内容]
     * - HEX 视图：紧凑格式 `AA CC 12 34 ...`，不带地址/ASCII 列（用户要求）
     * - TEXT 视图：原始字节按编码解码
     * - RX：蓝色文字；TX：绿色文字；时间戳暗显（v1.0.1 去掉了整行底色）
     */
    const writeData = useCallback(
      (data: Uint8Array, direction: Direction = "rx") => {
        const xterm = xtermRef.current;
        if (!xterm || data.length === 0) return;

        const ts = formatTimestamp(new Date());
        const fg = direction === "rx" ? RX_FG : TX_FG;
        const arrow = direction === "rx" ? "←" : "→";

        // 时间戳 + 方向（前缀：暗显 + 加粗）
        const header = `${DIM}${ts}${ANSI_RESET} ${BRIGHT}${fg}${arrow}${ANSI_RESET} `;
        xterm.write(header);

        // 内容
        if (viewMode === "hex") {
          // 用户要求：只显示 HEX 字节，不带地址/ASCII 列
          const hex = Array.from(data, byteHex).join(" ");
          xterm.write(`${fg}${hex}${ANSI_RESET}`);
        } else {
          // 文本视图：按编码解码
          let text: string;
          if (encoding === "gbk") {
            text = decodeGBK(data);
          } else {
            text = new TextDecoder("utf-8").decode(data);
          }
          xterm.write(`${fg}${text}${ANSI_RESET}`);
        }
        xterm.write("\r\n");
      },
      [viewMode, encoding],
    );

    useImperativeHandle(ref, () => ({
      writeData,
      clear: () => xtermRef.current?.clear(),
    }));

    return <div ref={terminalRef} className="h-full w-full" />;
  },
);

Terminal.displayName = "Terminal";
