/**
 * 状态栏：显示收发字节、连接状态、溢出计数
 */
import { useBufferStore } from "../stores/bufferStore";
import { useSerialStore } from "../stores/serialStore";
import { bytesToHuman } from "../utils/format";

export function StatusBar() {
  const rxBytes = useBufferStore((s) => s.rxBytes);
  const txBytes = useBufferStore((s) => s.txBytes);
  const overflowCount = useBufferStore((s) => s.overflowCount);

  const isOpen = useSerialStore((s) => s.isOpen);
  const disconnected = useSerialStore((s) => s.disconnected);
  const portName = useSerialStore((s) => s.portName);

  const statusText = disconnected
    ? "已断开"
    : isOpen
      ? `已连接 ${portName}`
      : "未连接";
  const statusColor = disconnected
    ? "text-red-400"
    : isOpen
      ? "text-green-400"
      : "text-gray-400";

  return (
    <div className="flex items-center justify-between px-4 py-2 bg-gray-800 border-t border-gray-700 text-sm">
      <div className="flex items-center gap-6">
        <span className={statusColor}>{statusText}</span>
        <span className="text-gray-400">
          <span className="text-blue-400">TX</span>: {bytesToHuman(txBytes)}
        </span>
        <span className="text-gray-400">
          <span className="text-yellow-400">RX</span>: {bytesToHuman(rxBytes)}
        </span>
        {overflowCount > 0 && (
          <span className="text-red-400">
            ⚠ 溢出 {overflowCount} 次
          </span>
        )}
      </div>
      <span className="text-gray-500">OhMySerial v0.1.0</span>
    </div>
  );
}
