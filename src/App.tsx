import { useEffect, useMemo, useRef, useState } from "react";
import { Terminal, TerminalHandle } from "./components/Terminal";
import { SerialToolbar } from "./components/SerialToolbar";
import { StatusBar } from "./components/StatusBar";
import { SendPanel, SendPanelHandle } from "./components/SendPanel";
import { PresetPanel } from "./components/PresetPanel";
import { HotkeyHelp } from "./components/HotkeyHelp";
import { useSerialStore } from "./stores/serialStore";
import { useBufferStore } from "./stores/bufferStore";
import { useUiStore } from "./stores/uiStore";
import { useHotkeys, Hotkey } from "./hooks/useHotkeys";
import { useThemeClasses } from "./hooks/useThemeClasses";
import { useConfigSync } from "./hooks/useConfigSync";

type ViewMode = "text" | "hex";

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>("text");
  const encoding = useSerialStore((s) => s.encoding);
  const terminalRef = useRef<TerminalHandle>(null);
  const sendRef = useRef<SendPanelHandle>(null);
  const t = useThemeClasses();

  // 启动时从 Rust 加载配置 + 订阅变化写盘
  useConfigSync();

  // 监听 Rust 推送的串口数据
  useEffect(() => {
    const unlistens: Array<() => void> = [];

    (async () => {
      const { listen } = await import("@tauri-apps/api/event");

      // 1) 串口数据
      const unData = await listen<number[]>("serial-data", (event) => {
        const payload = event.payload;
        if (Array.isArray(payload) && payload.length > 0) {
          terminalRef.current?.writeData(new Uint8Array(payload));
          useBufferStore.getState().incrementRx(payload.length);
        }
      });
      unlistens.push(unData);

      // 2) 设备断线
      const unDisc = await listen<string>("port-disconnected", (event) => {
        useSerialStore.getState().setDisconnected(true);
        useSerialStore.setState({ isOpen: false });
        // 终端写入提示
        const msg = `\r\n[系统] 设备已断开: ${event.payload}\r\n`;
        terminalRef.current?.writeData(new TextEncoder().encode(msg));
      });
      unlistens.push(unDisc);

      // 3) 自动重连进度（Rust 推送）
      const unRecon = await listen<{
        state: "started" | "attempt" | "succeeded" | "failed" | "cancelled";
        attempt: number;
        max_attempts: number;
        next_delay_ms: number;
        message: string;
      }>("reconnect-status", (event) => {
        const p = event.payload;
        if (p.state === "succeeded" || p.state === "failed" || p.state === "cancelled") {
          // 终态：保留 1.5s 让用户看到，然后清空
          useSerialStore.getState().setReconnect(p);
          setTimeout(() => {
            const cur = useSerialStore.getState().reconnect;
            if (cur && cur.state === p.state) {
              useSerialStore.getState().setReconnect(null);
            }
          }, 1500);
          // 成功时同步 isOpen/disconnected
          if (p.state === "succeeded") {
            useSerialStore.getState().setDisconnected(false);
            useSerialStore.setState({ isOpen: true });
            const ok = `\r\n[系统] ${p.message}\r\n`;
            terminalRef.current?.writeData(new TextEncoder().encode(ok));
          } else if (p.state === "failed") {
            const msg = `\r\n[系统] ${p.message}\r\n`;
            terminalRef.current?.writeData(new TextEncoder().encode(msg));
          }
        } else {
          useSerialStore.getState().setReconnect(p);
        }
      });
      unlistens.push(unRecon);
    })();

    return () => {
      unlistens.forEach((u) => u());
    };
  }, []);

  // 全局快捷键
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);

  // Esc/Shift+L 由 HotkeyHelp 或 SendPanel 自身处理；这里只列需要全局拦截的
  const hotkeys: Hotkey[] = useMemo(
    () => [
      {
        key: "l",
        ctrl: true,
        handler: () => terminalRef.current?.clear(),
        description: "清空终端",
      },
      {
        key: "k",
        ctrl: true,
        handler: () => sendRef.current?.focus(),
        description: "聚焦到发送输入框",
      },
      {
        key: "t",
        ctrl: true,
        handler: () => {
          // 循环切换：dark → light → system → dark
          const next = theme === "dark" ? "light" : theme === "light" ? "system" : "dark";
          setTheme(next);
        },
        description: "循环切换主题（暗 → 亮 → 跟随系统）",
      },
    ],
    [theme, setTheme],
  );

  useHotkeys(hotkeys);

  return (
    <div className={`flex flex-col h-screen ${t.bg.primary} ${t.text.primary}`}>
      {/* 标题栏 */}
      <div
        className={`flex items-center justify-between px-4 py-2 ${t.bg.secondary} border-b ${t.border.default}`}
      >
        <h1 className="text-lg font-semibold">🚀 OhMySerial</h1>
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-2">
            <span className="text-sm">视图:</span>
            <select
              value={viewMode}
              onChange={(e) => setViewMode(e.target.value as ViewMode)}
              className={`px-2 py-1 ${t.bg.tertiary} rounded ${t.text.primary} text-sm`}
            >
              <option value="text">文本</option>
              <option value="hex">HEX</option>
            </select>
          </label>
        </div>
      </div>

      {/* 串口工具栏 */}
      <SerialToolbar />

      {/* 终端区域 + 发送面板（左右分栏） */}
      <div className="flex-1 flex p-4 gap-4 min-h-0">
        <div
          className={`flex-1 ${t.bg.secondary} rounded-lg overflow-hidden border ${t.border.default}`}
        >
          <Terminal ref={terminalRef} viewMode={viewMode} encoding={encoding} />
        </div>
        <div className="w-80 flex-shrink-0 flex flex-col gap-2 min-h-0">
          <div className="h-1/2 min-h-0">
            <SendPanel ref={sendRef} />
          </div>
          <div className="h-1/2 min-h-0">
            <PresetPanel />
          </div>
        </div>
      </div>

      {/* 状态栏 */}
      <StatusBar />

      {/* 快捷键帮助浮层（F1 / ? 切换） */}
      <HotkeyHelp hotkeys={hotkeys} />
    </div>
  );
}

export default App;
