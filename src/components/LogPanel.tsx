/**
 * 日志面板（LogPanel）
 *
 * 抽屉式：父组件通过 open 控制显隐
 * - 顶栏：level 下拉 + keyword 输入 + "打开目录" + 关闭
 * - 中间：可滚动行列表（按时间正序）
 * - 底部：状态条
 *
 * 数据由 useLogPolling 自动注入 logStore
 */
import { useEffect, useRef, useState } from "react";
import { useThemeClasses } from "../hooks/useThemeClasses";
import { useLogStore, applyFilter, LevelFilter } from "../stores/logStore";
import { useLogPolling } from "../hooks/useLogPolling";

export interface LogPanelProps {
  /** 受控开关 */
  open: boolean;
  /** 关闭回调 */
  onClose: () => void;
}

const LEVEL_OPTIONS: { value: LevelFilter; label: string }[] = [
  { value: "ALL", label: "全部" },
  { value: "DEBUG", label: "Debug+" },
  { value: "INFO", label: "Info+" },
  { value: "WARN", label: "Warn+" },
  { value: "ERROR", label: "Error" },
];

const LEVEL_COLOR: Record<string, string> = {
  DEBUG: "text-gray-500 dark:text-gray-400",
  INFO: "text-blue-600 dark:text-blue-400",
  WARN: "text-yellow-600 dark:text-yellow-400",
  ERROR: "text-red-600 dark:text-red-400",
};

/** 格式化 lastFetchedAt 为 HH:MM:SS */
function formatTime(ms: number | null): string {
  if (ms === null) return "—";
  const d = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function LogPanel({ open, onClose }: LogPanelProps) {
  const t = useThemeClasses();
  const lines = useLogStore((s) => s.lines);
  const levelFilter = useLogStore((s) => s.levelFilter);
  const keyword = useLogStore((s) => s.keyword);
  const lastFetchedAt = useLogStore((s) => s.lastFetchedAt);
  const setLevelFilter = useLogStore((s) => s.setLevelFilter);
  const setKeyword = useLogStore((s) => s.setKeyword);
  const clear = useLogStore((s) => s.clear);

  // 仅在打开时拉取（节能）
  useLogPolling({ intervalMs: 2000, limit: 200, enabled: open });

  const [autoScroll, setAutoScroll] = useState(true);
  const listRef = useRef<HTMLDivElement>(null);

  // 自动滚到底
  useEffect(() => {
    if (autoScroll && listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [lines, autoScroll, open]);

  // 检测用户手动滚动 → 暂停自动滚动
  const onScroll = () => {
    const el = listRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 20;
    if (!atBottom && autoScroll) setAutoScroll(false);
    if (atBottom && !autoScroll) setAutoScroll(true);
  };

  const handleOpenDir = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("cmd_open_log_dir");
    } catch (e) {
      console.warn("打开日志目录失败:", e);
    }
  };

  const filtered = applyFilter(lines, levelFilter, keyword);

  if (!open) return null;

  return (
    <div
      className={`flex flex-col h-full ${t.bg.secondary} border-t ${t.border.default}`}
    >
      {/* 顶栏 */}
      <div
        className={`flex items-center gap-2 px-3 py-2 ${t.bg.tertiary} border-b ${t.border.default} flex-wrap`}
      >
        <span className="text-sm font-semibold">📋 日志</span>

        <select
          value={levelFilter}
          onChange={(e) => setLevelFilter(e.target.value as LevelFilter)}
          className={`px-2 py-1 text-xs ${t.bg.primary} ${t.text.primary} rounded border ${t.border.input}`}
          title="按级别过滤"
        >
          {LEVEL_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              级别: {o.label}
            </option>
          ))}
        </select>

        <input
          type="text"
          value={keyword}
          onChange={(e) => setKeyword(e.target.value)}
          placeholder="关键字 (message / target)"
          className={`flex-1 min-w-[120px] px-2 py-1 text-xs ${t.bg.primary} ${t.text.primary} rounded border ${t.border.input} focus:outline-none focus:border-blue-500`}
        />

        <label className="flex items-center gap-1 text-xs cursor-pointer">
          <input
            type="checkbox"
            checked={autoScroll}
            onChange={(e) => setAutoScroll(e.target.checked)}
          />
          自动滚
        </label>

        <button
          onClick={handleOpenDir}
          className={`px-2 py-1 text-xs ${t.bg.primary} hover:opacity-80 ${t.text.primary} rounded border ${t.border.input}`}
          title="用资源管理器打开日志目录"
        >
          📂 目录
        </button>

        <button
          onClick={() => clear()}
          className={`px-2 py-1 text-xs ${t.bg.primary} hover:opacity-80 ${t.text.primary} rounded border ${t.border.input}`}
          title="清空当前缓存（不影响磁盘文件）"
        >
          🗑 清空
        </button>

        <button
          onClick={onClose}
          className={`px-2 py-1 text-xs ${t.bg.primary} hover:opacity-80 ${t.text.primary} rounded border ${t.border.input}`}
          title="关闭日志面板 (F2)"
        >
          ✕
        </button>
      </div>

      {/* 行列表 */}
      <div
        ref={listRef}
        onScroll={onScroll}
        className="flex-1 overflow-y-auto px-2 py-1 font-mono text-xs leading-5"
      >
        {filtered.length === 0 ? (
          <div className={`text-center py-8 ${t.text.muted}`}>
            {lines.length === 0
              ? "暂无日志（按 F2 关闭/打开，等待 2s 自动拉取）"
              : "无匹配日志（调整过滤条件）"}
          </div>
        ) : (
          filtered.map((l, i) => (
            <div
              key={`${l.fullTimestamp}-${i}`}
              className="flex gap-2 hover:bg-gray-100 dark:hover:bg-gray-800 px-1 rounded"
            >
              <span className={`${t.text.muted} flex-shrink-0`}>
                {l.timestamp}
              </span>
              <span
                className={`flex-shrink-0 w-12 font-semibold ${LEVEL_COLOR[l.level] ?? t.text.primary}`}
              >
                {l.level}
              </span>
              <span className={`${t.text.muted} flex-shrink-0 max-w-[140px] truncate`}>
                [{l.target}]
              </span>
              <span className={`${t.text.primary} break-all whitespace-pre-wrap`}>
                {l.message}
              </span>
            </div>
          ))
        )}
      </div>

      {/* 底部状态 */}
      <div
        className={`flex items-center justify-between px-3 py-1 text-xs ${t.text.muted} ${t.bg.tertiary} border-t ${t.border.default}`}
      >
        <span>
          {filtered.length} / {lines.length} 行
          {levelFilter !== "ALL" && ` · 级别: ${levelFilter}+`}
          {keyword && ` · 关键字: "${keyword}"`}
        </span>
        <span>上次刷新: {formatTime(lastFetchedAt)}</span>
      </div>
    </div>
  );
}
