/**
 * 状态栏：显示收发字节、连接状态、溢出计数 + 日志目录入口
 *
 * 设计：rxBytes/txBytes 在 store 里 60Hz 累积（精度不丢），显示用 useRafValue 节流到 ~15Hz
 * 目的：避免 rx 60Hz 触发整树重渲染
 */
import { useEffect, useState } from "react";
import { useBufferStore } from "../stores/bufferStore";
import { useSerialStore } from "../stores/serialStore";
import { useRecorderStore } from "../stores/recorderStore";
import { bytesToHuman } from "../utils/format";
import { APP_VERSION } from "../utils/version";
import { useThemeClasses } from "../hooks/useThemeClasses";
import { useRafValue } from "../hooks/useRafValue";

/** 大文件警告阈值（500MB，Q15A） */
const LARGE_FILE_THRESHOLD = 500 * 1024 * 1024;

export function StatusBar() {
  // 源：60Hz 累积（store 端）
  const rxRaw = useBufferStore((s) => s.rxBytes);
  const txRaw = useBufferStore((s) => s.txBytes);
  const overflowCount = useBufferStore((s) => s.overflowCount);
  // 显：rAF 节流到 ~15Hz
  const rxBytes = useRafValue(rxRaw);
  const txBytes = useRafValue(txRaw);

  const isOpen = useSerialStore((s) => s.isOpen);
  const disconnected = useSerialStore((s) => s.disconnected);
  const portName = useSerialStore((s) => s.portName);
  const reconnect = useSerialStore((s) => s.reconnect);
  const cancelReconnect = useSerialStore((s) => s.cancelReconnect);
  const t = useThemeClasses();

  // v1.2.0 录制指示（与 TX/RX 一致节流）
  const isRecording = useRecorderStore((s) => s.isRecording);
  const recBytesRaw = useRecorderStore((s) => s.bytesWritten);
  const recBytes = useRafValue(recBytesRaw);

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

  const statusText = reconnect
    ? `🔄 ${reconnect.message}`
    : disconnected
      ? "已断开"
      : isOpen
        ? `已连接 ${portName}`
        : "未连接";
  const statusColor = reconnect
    ? "text-yellow-600 dark:text-yellow-400"
    : disconnected
      ? t.status.disconnected
      : isOpen
        ? t.status.connected
        : t.text.secondary;

  return (
    <div
      className={`flex items-center justify-between px-4 py-2 min-w-0 overflow-hidden ${t.bg.secondary} border-t ${t.border.default} text-sm`}
    >
      <div className="flex items-center gap-6 min-w-0 flex-shrink">
        <span className={statusColor}>{statusText}</span>
        {reconnect &&
          (reconnect.state === "started" || reconnect.state === "attempt") && (
            <button
              onClick={() => void cancelReconnect()}
              className={`text-xs px-2 py-0.5 ${t.bg.tertiary} hover:opacity-80 rounded transition-colors`}
            >
              取消重连
            </button>
          )}
        <span className={t.text.muted}>
          <span className={t.status.tx}>TX</span>: {bytesToHuman(txBytes)}
        </span>
        <span className={t.text.muted}>
          <span className={t.status.rx}>RX</span>: {bytesToHuman(rxBytes)}
        </span>
        {isRecording && (
          <span
            className="flex items-center gap-1 text-red-500"
            data-rec-indicator
          >
            <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            REC {bytesToHuman(recBytes)}
            {recBytes > LARGE_FILE_THRESHOLD && (
              <span className="text-orange-500 ml-1" title="录制文件超过 500MB">
                ⚠
              </span>
            )}
          </span>
        )}
        {overflowCount > 0 && (
          <span className={t.status.warning}>⚠ 溢出 {overflowCount} 次</span>
        )}
      </div>
      <div className="flex items-center gap-3 min-w-0 flex-shrink-0 truncate">
        {logDir && (
          <span
            className={`text-xs ${t.text.muted} truncate max-w-[300px]`}
            title={`日志目录: ${logDir}\n保留最近 7 天`}
          >
            📝 {logDir}
          </span>
        )}
        <span className={`${t.text.muted} whitespace-nowrap`}>
          OhMySerial v{APP_VERSION}
        </span>
      </div>
    </div>
  );
}
