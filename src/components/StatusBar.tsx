/**
 * 状态栏：显示收发字节、连接状态、溢出计数 + 日志目录入口
 */
import { useEffect, useState } from "react";
import { useBufferStore } from "../stores/bufferStore";
import { useSerialStore } from "../stores/serialStore";
import { bytesToHuman } from "../utils/format";
import { useThemeClasses } from "../hooks/useThemeClasses";

export function StatusBar() {
  const rxBytes = useBufferStore((s) => s.rxBytes);
  const txBytes = useBufferStore((s) => s.txBytes);
  const overflowCount = useBufferStore((s) => s.overflowCount);

  const isOpen = useSerialStore((s) => s.isOpen);
  const disconnected = useSerialStore((s) => s.disconnected);
  const portName = useSerialStore((s) => s.portName);
  const t = useThemeClasses();

  const [logDir, setLogDir] = useState<string | null>(null);

  // 拉一次日志目录（启动时）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const dir = await invoke<string>("cmd_get_log_dir");
        if (!cancelled) setLogDir(dir);
      } catch (e) {
        // 非 Tauri 环境（如纯前端测试）会失败，忽略
        console.debug("cmd_get_log_dir failed:", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const statusText = disconnected
    ? "已断开"
    : isOpen
      ? `已连接 ${portName}`
      : "未连接";
  const statusColor = disconnected
    ? t.status.disconnected
    : isOpen
      ? t.status.connected
      : t.text.secondary;

  return (
    <div
      className={`flex items-center justify-between px-4 py-2 ${t.bg.secondary} border-t ${t.border.default} text-sm`}
    >
      <div className="flex items-center gap-6">
        <span className={statusColor}>{statusText}</span>
        <span className={t.text.muted}>
          <span className={t.status.tx}>TX</span>: {bytesToHuman(txBytes)}
        </span>
        <span className={t.text.muted}>
          <span className={t.status.rx}>RX</span>: {bytesToHuman(rxBytes)}
        </span>
        {overflowCount > 0 && (
          <span className={t.status.warning}>⚠ 溢出 {overflowCount} 次</span>
        )}
      </div>
      <div className="flex items-center gap-3">
        {logDir && (
          <span
            className={`text-xs ${t.text.muted} truncate max-w-[300px]`}
            title={`日志目录: ${logDir}\n保留最近 7 天`}
          >
            📝 {logDir}
          </span>
        )}
        <span className={t.text.muted}>OhMySerial v0.4.0</span>
      </div>
    </div>
  );
}
