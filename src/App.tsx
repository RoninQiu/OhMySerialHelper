import { useEffect, useMemo, useRef, useState } from "react";
import { Terminal, TerminalHandle } from "./components/Terminal";
import { SerialToolbar } from "./components/SerialToolbar";
import { StatusBar } from "./components/StatusBar";
import { SendPanel, SendPanelHandle } from "./components/SendPanel";
import { PresetPanel } from "./components/PresetPanel";
import { HotkeyHelp } from "./components/HotkeyHelp";
import { LogPanel } from "./components/LogPanel";
import { useSerialStore } from "./stores/serialStore";
import { useUiStore } from "./stores/uiStore";
import { useHotkeys, Hotkey } from "./hooks/useHotkeys";
import { useThemeClasses } from "./hooks/useThemeClasses";
import { useConfigSync } from "./hooks/useConfigSync";

type ViewMode = "text" | "hex";

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>("text");
  const [showLogPanel, setShowLogPanel] = useState(false);
  const encoding = useSerialStore((s) => s.encoding);
  const terminalRef = useRef<TerminalHandle>(null);
  const sendRef = useRef<SendPanelHandle>(null);
  const t = useThemeClasses();

  // 启动时从 Rust 加载配置 + 订阅变化写盘
  useConfigSync();

  // 绑定串口数据回调：openPort 创建的 Channel.onmessage 会调到这里写终端
  // rxBytes 由 store 内部 incrementRx 统计
  useEffect(() => {
    useSerialStore.getState().setDataHandler((data) => {
      terminalRef.current?.writeData(data);
    });
    return () => {
      useSerialStore.getState().setDataHandler(null);
    };
  }, []);

  // 监听 Rust 推送的设备断线 + 重连进度（串口数据已改走 Channel 零拷贝）
  useEffect(() => {
    const unlistens: Array<() => void> = [];

    (async () => {
      const { listen } = await import("@tauri-apps/api/event");

      // 1) 设备断线
      const unDisc = await listen<string>("port-disconnected", (event) => {
        useSerialStore.getState().setDisconnected(true);
        useSerialStore.setState({ isOpen: false });
        // 终端写入提示
        const msg = `\r\n[系统] 设备已断开: ${event.payload}\r\n`;
        terminalRef.current?.writeData(new TextEncoder().encode(msg));
      });
      unlistens.push(unDisc);

      // 2) 自动重连进度（Rust 推送）
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
      {
        key: "F2",
        handler: () => setShowLogPanel((v) => !v),
        description: "切换日志面板",
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
          <button
            onClick={() => setShowLogPanel((v) => !v)}
            className={`px-3 py-1 text-sm rounded ${
              showLogPanel
                ? "bg-blue-500 text-white"
                : `${t.bg.tertiary} ${t.text.primary} hover:opacity-80`
            }`}
            title="切换日志面板 (F2)"
          >
            📋 日志
          </button>
        </div>
      </div>

      {/* 串口工具栏 */}
      <SerialToolbar />

      {/* 中间区：终端/发送（左右）+ 日志面板（底部） */}
      <div className={`flex-1 flex p-4 gap-4 min-h-0 ${showLogPanel ? "flex-col" : ""}`}>
        {/* 终端 + 发送面板 */}
        <div
          className={`flex ${showLogPanel ? "h-2/3" : "h-full"} flex-1 gap-4 min-h-0`}
        >
          <div
            className={`flex-1 ${t.bg.secondary} rounded-lg overflow-hidden border ${t.border.default}`}
          >
            <Terminal ref={terminalRef} viewMode={viewMode} encoding={encoding} />
          </div>
          <div className="w-80 flex-shrink-0 flex flex-col gap-2 min-h-0">
            <div className="h-1/2 min-h-0">
              <SendPanel
                ref={sendRef}
                onSent={(data) => terminalRef.current?.writeData(data, "tx")}
              />
            </div>
            <div className="h-1/2 min-h-0">
              <PresetPanel />
            </div>
          </div>
        </div>

        {/* 日志面板（底部抽屉） */}
        {showLogPanel && (
          <div className="h-1/3 min-h-0 rounded-lg overflow-hidden border">
            <LogPanel open={showLogPanel} onClose={() => setShowLogPanel(false)} />
          </div>
        )}
      </div>

      {/* 状态栏 */}
      <StatusBar />

      {/* 快捷键帮助浮层（F1 / ? 切换） */}
      <HotkeyHelp hotkeys={hotkeys} />
    </div>
  );
}

export default App;
