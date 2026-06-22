import { useEffect, useRef, useCallback, forwardRef, useImperativeHandle } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { decodeGBK } from "../utils/encoding";
import { APP_VERSION } from "../utils/version";
import { useUiStore } from "../stores/uiStore";
import { useConfigStore } from "../stores/configStore";
import { useRecorderStore } from "../stores/recorderStore";
import { resolveFontFamily } from "../utils/fonts";
import {
  formatTimestamp,
  byteHex,
  formatLine,
  type Direction,
  type ViewMode,
  type Encoding,
} from "../utils/terminalFormat";

// re-export for backward compat（其它文件可能 import 自 Terminal.tsx）
export { formatTimestamp, byteHex };

export interface TerminalHandle {
  writeData: (data: Uint8Array, direction?: Direction) => void;
  clear: () => void;
}

interface TerminalProps {
  viewMode: ViewMode;
  encoding: Encoding;
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

export const Terminal = forwardRef<TerminalHandle, TerminalProps>(
  ({ viewMode, encoding }, ref) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<XTerm | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const resolvedTheme = useUiStore((s) => s.resolvedTheme());
    const fontSize = useConfigStore((s) => s.config.font_size);
    const fontFamily = useConfigStore((s) => s.config.font_family);

    // 初始化 xterm
    useEffect(() => {
      if (!terminalRef.current) return;

      const xterm = new XTerm({
        theme: XTERM_THEMES.dark,
        cursorBlink: true,
        fontSize,
        fontFamily: resolveFontFamily(fontFamily),
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

    // 字号变化（reviewer #1 rAF 防阻塞；#2 显式 refresh；fix bug: 必须 fitAddon.fit() 重算 cols）
    useEffect(() => {
      if (!xtermRef.current) return;
      const rafId = requestAnimationFrame(() => {
        if (xtermRef.current && fitAddonRef.current) {
          xtermRef.current.options.fontSize = fontSize;
          fitAddonRef.current.fit(); // ★ cellWidth 变 → cols 需重算，否则单行总宽 > 容器 → 横向滚动条
          xtermRef.current.refresh(0, xtermRef.current.rows - 1);
        }
      });
      return () => cancelAnimationFrame(rafId);
    }, [fontSize]);

    // 字体变化（同上：cellWidth 同样变 → 必须 fit）
    useEffect(() => {
      if (!xtermRef.current) return;
      const rafId = requestAnimationFrame(() => {
        if (xtermRef.current && fitAddonRef.current) {
          xtermRef.current.options.fontFamily = resolveFontFamily(fontFamily);
          fitAddonRef.current.fit(); // ★ 同上：cols 需重算
          xtermRef.current.refresh(0, xtermRef.current.rows - 1);
        }
      });
      return () => cancelAnimationFrame(rafId);
    }, [fontFamily]);

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

        // v1.2.0：若在录制中，同步推一行纯文本给 Rust Recorder（去 ANSI）
        // 不阻塞 xterm 渲染：invoke 异步失败仅 console.warn
        if (useRecorderStore.getState().isRecording) {
          const line = formatLine(data, direction, viewMode, encoding);
          void import("@tauri-apps/api/core")
            .then(({ invoke }) =>
              invoke("cmd_write_recorder_line", { line }).catch((e) =>
                console.warn("recorder write_line failed:", e),
              ),
            )
            .catch((e) => console.warn("recorder invoke failed:", e));
        }
      },
      [viewMode, encoding],
    );

    useImperativeHandle(ref, () => ({
      writeData,
      clear: () => xtermRef.current?.clear(),
    }));

    // ★ min-w-0 / min-h-0 / overflow-hidden 防御：
    //   xterm 内部 .xterm-screen 的 canvas 元素会用 inline width/height 属性
    //   撑出 cells×cellWidth 实际尺寸。在 flexbox 中，flex 子项默认
    //   min-width: auto = 内容最小宽度，会被 xterm 内部撑大 → 挤压兄弟
    //   flex-1 的右侧栏（甚至挤出视口）。min-w-0 让 flex 允许收缩到比
    //   内容小，overflow-hidden 裁掉 xterm 内部溢出。xterm 自己的
    //   .xterm-viewport 内部仍有 overflow: auto 滚动条，单行/单屏可滚。
    return <div ref={terminalRef} className="h-full w-full min-w-0 min-h-0 overflow-hidden" />;
  },
);

Terminal.displayName = "Terminal";
