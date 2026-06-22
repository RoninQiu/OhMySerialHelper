import { useEffect, useState } from "react";
import { useSerialStore } from "../stores/serialStore";
import { useBufferStore, BUFFER_SIZES } from "../stores/bufferStore";
import { useUiStore, Theme } from "../stores/uiStore";
import { useThemeClasses } from "../hooks/useThemeClasses";
import { useConfigStore } from "../stores/configStore";
import { useRecorderStore, type RecorderSummary } from "../stores/recorderStore";
import { FONT_SIZE_RANGE } from "../utils/fonts";
import { FontPicker } from "./FontPicker";

const BAUD_RATES = [
  9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
];

export function SerialToolbar() {
  const {
    isOpen,
    disconnected,
    portName,
    baudRate,
    encoding,
    openPort,
    closePort,
    setEncoding,
    setBaudRate,
  } = useSerialStore();
  const { bufferSize, setBufferSize } = useBufferStore();
  const { theme, setTheme } = useUiStore();
  const fontSize = useConfigStore((s) => s.config.font_size);
  const setFontSize = useConfigStore((s) => s.setFontSize);
  const t = useThemeClasses();

  const [ports, setPorts] = useState<
    {
      name: string;
      port_type: string;
      manufacturer?: string | null;
      product?: string | null;
    }[]
  >([]);
  const [selectedPort, setSelectedPort] = useState(portName);
  const [error, setError] = useState<string | null>(null);

  // Fetch available ports on mount
  useEffect(() => {
    const fetchPorts = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const portList = await invoke<
          {
            name: string;
            port_type: string;
            manufacturer?: string | null;
            product?: string | null;
          }[]
        >("cmd_list_ports");
        setPorts(portList);
      } catch (e) {
        console.error("Failed to list ports:", e);
      }
    };

    fetchPorts();
    // Poll for new ports every 5 seconds
    const interval = setInterval(fetchPorts, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleOpen = async () => {
    if (!selectedPort) return;
    setError(null);
    try {
      await openPort(selectedPort, baudRate);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message ?? String(e);
      setError(msg);
      console.error("Failed to open port:", e);
    }
  };

  const handleClose = async () => {
    setError(null);
    try {
      await closePort();
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message ?? String(e);
      setError(msg);
      console.error("Failed to close port:", e);
    }
  };

  return (
    <div className={`flex items-center gap-4 p-2 flex-wrap min-w-0 ${t.bg.secondary}`}>
      {/* Port Selection */}
      <select
        value={selectedPort}
        onChange={(e) => setSelectedPort(e.target.value)}
        disabled={isOpen}
        className={`px-2 py-1 rounded ${t.bg.tertiary} ${t.text.primary}`}
      >
        <option value="">选择串口</option>
        {ports.map((port) => {
          // v1.0.1：识别不到芯片时不显示后缀；带 manufacturer 时附加显示
          const chip = port.port_type;
          const mfr = port.manufacturer?.trim();
          const label = chip
            ? mfr
              ? `${port.name} (${chip} · ${mfr})`
              : `${port.name} (${chip})`
            : port.name;
          return (
            <option key={port.name} value={port.name}>
              {label}
            </option>
          );
        })}
      </select>

      {/* Baud Rate Selection */}
      <select
        value={baudRate}
        onChange={(e) => setBaudRate(Number(e.target.value))}
        disabled={isOpen}
        className={`px-2 py-1 rounded ${t.bg.tertiary} ${t.text.primary}`}
      >
        {BAUD_RATES.map((rate) => (
          <option key={rate} value={rate}>
            {rate}
          </option>
        ))}
      </select>

      {/* Encoding Selection */}
      <select
        value={encoding}
        onChange={(e) => setEncoding(e.target.value as "utf8" | "gbk")}
        className={`px-2 py-1 rounded ${t.bg.tertiary} ${t.text.primary}`}
      >
        <option value="utf8">UTF-8</option>
        <option value="gbk">GBK</option>
      </select>

      {/* Theme Selection */}
      <select
        value={theme}
        onChange={(e) => setTheme(e.target.value as Theme)}
        className={`px-2 py-1 rounded ${t.bg.tertiary} ${t.text.primary}`}
        title="主题（Ctrl+T 循环切换）"
      >
        <option value="dark">深色</option>
        <option value="light">浅色</option>
        <option value="system">跟随系统</option>
      </select>

      {/* Font Size Stepper (v1.1.0) */}
      <div className="inline-flex items-center border rounded dark:border-gray-700 text-sm">
        <button
          type="button"
          onClick={() => setFontSize(fontSize - FONT_SIZE_RANGE.step)}
          disabled={fontSize <= FONT_SIZE_RANGE.min}
          className="px-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-50"
          title="减小字号 (Ctrl+-)"
        >
          A−
        </button>
        <button
          type="button"
          onClick={() => setFontSize(FONT_SIZE_RANGE.default)}
          className="px-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 min-w-[80px]"
          title="点击重置到 14px (Ctrl+0)"
        >
          {fontSize}px
        </button>
        <button
          type="button"
          onClick={() => setFontSize(fontSize + FONT_SIZE_RANGE.step)}
          disabled={fontSize >= FONT_SIZE_RANGE.max}
          className="px-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-50"
          title="增大字号 (Ctrl++)"
        >
          A+
        </button>
      </div>

      {/* Font Picker (v1.1.0) */}
      <FontPicker />

      {/* Buffer Size Selection */}
      <select
        value={bufferSize}
        onChange={(e) => setBufferSize(Number(e.target.value))}
        className={`px-2 py-1 rounded ${t.bg.tertiary} ${t.text.primary}`}
      >
        {BUFFER_SIZES.map((size) => (
          <option key={size} value={size}>
            {(size / 1024 / 1024).toFixed(0)}MB 缓冲
          </option>
        ))}
      </select>

      {/* Open/Close Button */}
      <button
        onClick={isOpen ? handleClose : handleOpen}
        disabled={!isOpen && !selectedPort}
        className={`px-4 py-1 rounded font-medium transition-colors ${
          isOpen
            ? "bg-red-500 hover:bg-red-600 text-white"
            : "bg-blue-500 hover:bg-blue-600 text-white disabled:opacity-50"
        }`}
      >
        {isOpen ? "关闭串口" : "打开串口"}
      </button>

      {/* Recording Button（v1.2.0） */}
      <RecordingButton />

      {/* Settings Button（v1.2.0，触发 App 渲染 SettingsPanel） */}
      <button
        type="button"
        onClick={() => {
          window.dispatchEvent(new CustomEvent("oh-my-serial:toggle-settings"));
        }}
        className={`px-3 py-1 text-sm ${t.bg.tertiary} hover:opacity-80 ${t.text.primary} rounded`}
        title="设置"
      >
        ⚙
      </button>

      {/* Status Indicator */}
      <div className="flex items-center gap-2">
        <div
          className={`w-2 h-2 rounded-full ${
            disconnected
              ? "bg-red-500"
              : isOpen
                ? "bg-green-500"
                : "bg-gray-400 dark:bg-gray-500"
          }`}
          title={
            disconnected
              ? "设备已断开"
              : isOpen
                ? "已连接"
                : "未连接"
          }
        />
        <span
          className={`text-sm ${
            disconnected ? t.status.disconnected : t.text.muted
          }`}
        >
          {disconnected
            ? `已断开 ${portName}`
            : isOpen
              ? `已连接 ${portName}`
              : "未连接"}
        </span>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="basis-full mt-2 px-3 py-1 rounded bg-red-100 text-red-700 dark:bg-red-900/50 dark:text-red-200 text-sm">
          ⚠ {error}
        </div>
      )}
    </div>
  );
}

/**
 * RecordingButton（v1.2.0）
 * - 点击"开始录制"：根据 configStore.prompt_save_dialog 决定弹对话框或用默认路径
 * - 点击"停止录制"：flush + 显示 summary（KB / 耗时）
 * - 文件名建议：`capture-{port}-{YYYYMMDD-HHmmss}.txt`
 */
function RecordingButton() {
  const t = useThemeClasses();
  const isRecording = useRecorderStore((s) => s.isRecording);
  const startRecording = useRecorderStore((s) => s.startRecording);
  const stopRecording = useRecorderStore((s) => s.stopRecording);
  const { isOpen, portName, baudRate, dataBits, stopBits, parity } = useSerialStore();
  const promptSaveDialog = useConfigStore((s) => s.config.prompt_save_dialog);
  const defaultCapturePath = useConfigStore((s) => s.config.default_capture_path);

  const handleClick = async () => {
    try {
      if (isRecording) {
        const summary: RecorderSummary | null = await stopRecording();
        if (summary) {
          const kb = (summary.bytes_written / 1024).toFixed(1);
          const sec = (summary.duration_ms / 1000).toFixed(1);
          console.info(
            `[recorder] 录制完成: ${kb} KB / ${sec}s → ${summary.path}`,
          );
        }
        return;
      }
      // 开始录制
      let path: string;
      if (promptSaveDialog || !defaultCapturePath) {
        // 弹文件对话框
        const { save } = await import("@tauri-apps/plugin-dialog");
        const stamp = new Date()
          .toISOString()
          .replace(/[-:T]/g, "")
          .slice(0, 15); // YYYYMMDDHHmmss
        const defaultName = `capture-${portName || "port"}-${stamp}.txt`;
        const defaultPath = defaultCapturePath
          ? `${defaultCapturePath.replace(/[/\\]+$/, "")}/${defaultName}`
          : defaultName;
        const picked = await save({
          defaultPath,
          filters: [{ name: "Text", extensions: ["txt"] }],
        });
        if (!picked) return; // 用户取消
        path = picked;
      } else {
        // 用默认路径 + 自动文件名
        const stamp = new Date()
          .toISOString()
          .replace(/[-:T]/g, "")
          .slice(0, 15);
        const sep = defaultCapturePath.includes("\\") ? "\\" : "/";
        path = `${defaultCapturePath.replace(/[/\\]+$/, "")}${sep}capture-${portName || "port"}-${stamp}.txt`;
      }
      await startRecording(path, portName, baudRate, dataBits, stopBits, parity);
    } catch (e) {
      console.error("toggle recording failed:", e);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={!isOpen}
      title={isRecording ? "停止录制" : "开始录制"}
      className={`px-3 py-1 text-sm rounded font-medium transition-colors ${
        isRecording
          ? "bg-red-600 hover:bg-red-700 text-white animate-pulse"
          : `${t.bg.tertiary} ${t.text.primary} hover:opacity-80 disabled:opacity-50`
      }`}
    >
      {isRecording ? "⏹ 停止录制" : "⏺ 录制"}
    </button>
  );
}
