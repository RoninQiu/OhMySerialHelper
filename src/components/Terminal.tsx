import { useEffect, useRef, useCallback, forwardRef, useImperativeHandle } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { bytesToHex } from "../utils/hex";
import { decodeGBK } from "../utils/encoding";
import { useUiStore } from "../stores/uiStore";

export interface TerminalHandle {
  writeData: (data: Uint8Array) => void;
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

      xterm.write("OhMySerial v0.1.0\r\n");
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

    // 写入数据
    const writeData = useCallback(
      (data: Uint8Array) => {
        const xterm = xtermRef.current;
        if (!xterm || data.length === 0) return;

        if (viewMode === "hex") {
          // HEX 视图：每行 16 字节，带地址前缀
          const lines: string[] = [];
          for (let i = 0; i < data.length; i += 16) {
            const chunk = data.slice(i, i + 16);
            const addr = i.toString(16).padStart(8, "0");
            const hex = bytesToHex(chunk).padEnd(48, " ");
            const ascii = Array.from(chunk)
              .map((b) => (b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : "."))
              .join("");
            lines.push(`${addr}  ${hex}  ${ascii}\r\n`);
          }
          xterm.write(lines.join(""));
        } else {
          // 文本视图：按编码解码
          if (encoding === "gbk") {
            xterm.write(decodeGBK(data));
          } else {
            // TextEncoder 接受 Uint8Array 直接输出 UTF-8
            xterm.write(data);
          }
        }
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
