import { useEffect, useRef, useCallback } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { bytesToHex } from "../utils/hex";
import { decodeGBK } from "../utils/encoding";

interface TerminalProps {
  onData?: (data: Uint8Array) => void;
  viewMode: "text" | "hex";
  encoding: "utf8" | "gbk";
}

export function Terminal({ onData, viewMode, encoding }: TerminalProps) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  // Initialize xterm
  useEffect(() => {
    if (!terminalRef.current) return;

    const xterm = new XTerm({
      theme: {
        background: "#1f2937",
        foreground: "#f3f4f6",
        cursor: "#3b82f6",
        cursorAccent: "#1f2937",
        selectionBackground: "#3b82f680",
      },
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

    // Write welcome message
    xterm.write("OhMySerial v0.1.0\r\n");
    xterm.write("=================\r\n\r\n");
    xterm.write("串口调试助手已就绪\r\n");
    xterm.write("请选择串口并点击连接...\r\n\r\n");

    return () => {
      xterm.dispose();
    };
  }, []);

  // Handle resize
  useEffect(() => {
    const handleResize = () => {
      fitAddonRef.current?.fit();
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  // Write data to terminal
  const writeData = useCallback(
    (data: Uint8Array) => {
      const xterm = xtermRef.current;
      if (!xterm) return;

      if (viewMode === "hex") {
        // Hex mode: display as hex dump
        const hex = bytesToHex(data);
        xterm.write(hex);
      } else {
        // Text mode: decode based on encoding
        if (encoding === "gbk") {
          const text = decodeGBK(data);
          xterm.write(text);
        } else {
          xterm.write(data);
        }
      }
    },
    [viewMode, encoding],
  );

  // Expose writeData for external use
  useEffect(() => {
    if (onData) {
      // This would be used for sending data, not receiving
    }
  }, [onData]);

  return <div ref={terminalRef} className="h-full w-full" />;
}

// Utility function to format hex for xterm display
export function formatHexDump(
  data: Uint8Array,
  bytesPerLine: number = 16,
): string {
  const lines: string[] = [];
  for (let i = 0; i < data.length; i += bytesPerLine) {
    const chunk = data.slice(i, i + bytesPerLine);
    const hex = bytesToHex(chunk);
    lines.push(hex);
  }
  return lines.join("\r\n");
}
