import { useEffect, useRef, useState } from "react";
import { Terminal, TerminalHandle } from "./components/Terminal";
import { SerialToolbar } from "./components/SerialToolbar";
import { StatusBar } from "./components/StatusBar";
import { SendPanel } from "./components/SendPanel";
import { PresetPanel } from "./components/PresetPanel";
import { useSerialStore } from "./stores/serialStore";
import { useBufferStore } from "./stores/bufferStore";
import { useUiStore } from "./stores/uiStore";
import { useHotkeys } from "./hooks/useHotkeys";

type ViewMode = "text" | "hex";
type Encoding = "utf8" | "gbk";

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>("text");
  const encoding = useSerialStore((s) => s.encoding);
  const terminalRef = useRef<TerminalHandle>(null);

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
    })();

    return () => {
      unlistens.forEach((u) => u());
    };
  }, []);

  // 全局快捷键
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  useHotkeys([
    {
      key: "l",
      ctrl: true,
      handler: () => terminalRef.current?.clear(),
    },
    {
      key: "t",
      ctrl: true,
      handler: () => {
        // 循环切换：dark → light → system → dark
        const next = theme === "dark" ? "light" : theme === "light" ? "system" : "dark";
        setTheme(next);
      },
    },
  ]);

  return (
    <div className="flex flex-col h-screen bg-gray-900 text-gray-100">
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-4 py-2 bg-gray-800 border-b border-gray-700">
        <h1 className="text-lg font-semibold">🚀 OhMySerial</h1>
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-2">
            <span className="text-sm">视图:</span>
            <select
              value={viewMode}
              onChange={(e) => setViewMode(e.target.value as ViewMode)}
              className="px-2 py-1 bg-gray-700 rounded text-white text-sm"
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
        <div className="flex-1 bg-gray-800 rounded-lg overflow-hidden border border-gray-700">
          <Terminal ref={terminalRef} viewMode={viewMode} encoding={encoding} />
        </div>
        <div className="w-80 flex-shrink-0 flex flex-col gap-2 min-h-0">
          <div className="h-1/2 min-h-0">
            <SendPanel />
          </div>
          <div className="h-1/2 min-h-0">
            <PresetPanel />
          </div>
        </div>
      </div>

      {/* 状态栏 */}
      <StatusBar />
    </div>
  );
}

export default App;
