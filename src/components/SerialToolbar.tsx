import { useEffect, useState } from "react";
import { useSerialStore } from "../stores/serialStore";
import { useBufferStore, BUFFER_SIZES } from "../stores/bufferStore";

const BAUD_RATES = [
  9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
];

export function SerialToolbar() {
  const {
    isOpen,
    portName,
    baudRate,
    encoding,
    openPort,
    closePort,
    setEncoding,
    setBaudRate,
  } = useSerialStore();
  const { bufferSize, setBufferSize } = useBufferStore();

  const [ports, setPorts] = useState<{ name: string; port_type: string }[]>([]);
  const [selectedPort, setSelectedPort] = useState(portName);
  const [error, setError] = useState<string | null>(null);

  // Fetch available ports on mount
  useEffect(() => {
    const fetchPorts = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const portList = await invoke<{ name: string; port_type: string }[]>("cmd_list_ports");
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
    <div className="flex items-center gap-4 p-2 bg-slate-800">
      {/* Port Selection */}
      <select
        value={selectedPort}
        onChange={(e) => setSelectedPort(e.target.value)}
        disabled={isOpen}
        className="px-2 py-1 rounded bg-slate-700 text-white"
      >
        <option value="">选择串口</option>
        {ports.map((port) => (
          <option key={port.name} value={port.name}>
            {port.name} ({port.port_type})
          </option>
        ))}
      </select>

      {/* Baud Rate Selection */}
      <select
        value={baudRate}
        onChange={(e) => setBaudRate(Number(e.target.value))}
        disabled={isOpen}
        className="px-2 py-1 rounded bg-slate-700 text-white"
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
        className="px-2 py-1 rounded bg-slate-700 text-white"
      >
        <option value="utf8">UTF-8</option>
        <option value="gbk">GBK</option>
      </select>

      {/* Buffer Size Selection */}
      <select
        value={bufferSize}
        onChange={(e) => setBufferSize(Number(e.target.value))}
        className="px-2 py-1 rounded bg-slate-700 text-white"
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

      {/* Status Indicator */}
      <div className="flex items-center gap-2">
        <div className={`w-2 h-2 rounded-full ${isOpen ? "bg-green-500" : "bg-gray-500"}`} />
        <span className="text-sm text-gray-400">
          {isOpen ? `已连接 ${portName}` : "未连接"}
        </span>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="basis-full mt-2 px-3 py-1 rounded bg-red-900/50 text-red-200 text-sm">
          ⚠ {error}
        </div>
      )}
    </div>
  );
}
