/**
 * SettingsPanel - 设置弹窗（v1.2.0）
 *
 * 设计：
 * - 照搬 HotkeyHelp 的 Modal 模式（fixed inset-0 z-50 bg-black/40）
 * - Esc / 点击背景关闭
 * - 字段：默认保存路径 + "每次录制时弹文件对话框" toggle
 * - 未来可扩展更多设置（HEX 分隔符、自动滚动等）
 *
 * 持久化：
 * - default_capture_path / prompt_save_dialog 走 useConfigStore → Rust cmd_save_config
 */

import { useEffect } from "react";
import { useConfigStore } from "../stores/configStore";
import { useThemeClasses } from "../hooks/useThemeClasses";

export interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsPanel({ open, onClose }: SettingsPanelProps) {
  const t = useThemeClasses();
  const config = useConfigStore((s) => s.config);
  const save = useConfigStore((s) => s.save);
  const setDefaultPath = (v: string) =>
    useConfigStore.setState((s) => ({
      config: { ...s.config, default_capture_path: v },
    }));
  const setPromptDialog = (v: boolean) =>
    useConfigStore.setState((s) => ({
      config: { ...s.config, prompt_save_dialog: v },
    }));

  // Esc 关闭
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const handleBrowse = async () => {
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const path = await openDialog({
        directory: true,
        multiple: false,
        title: "选择默认保存目录",
      });
      if (typeof path === "string") setDefaultPath(path);
    } catch (e) {
      console.warn("browse folder failed:", e);
    }
  };

  const handleSave = async () => {
    try {
      await save();
      onClose();
    } catch (e) {
      console.warn("save config failed:", e);
    }
  };

  return (
    <div
      data-settings-backdrop
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className={`rounded-lg shadow-2xl p-6 w-[480px] max-w-[90vw] ${t.bg.primary} border ${t.border.default}`}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className={`text-lg font-semibold mb-4 ${t.text.primary}`}>
          ⚙ 设置
        </h2>

        <div className="space-y-4">
          {/* 默认保存路径 */}
          <div>
            <label className={`block text-sm font-medium mb-1 ${t.text.primary}`}>
              录制默认保存路径
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={config.default_capture_path}
                onChange={(e) => setDefaultPath(e.target.value)}
                placeholder="留空则每次录制时弹文件对话框"
                className={`flex-1 px-2 py-1 text-sm ${t.bg.tertiary} ${t.text.primary} rounded border ${t.border.input} focus:outline-none focus:border-blue-500`}
              />
              <button
                type="button"
                onClick={handleBrowse}
                className={`px-3 py-1 text-sm ${t.bg.tertiary} hover:opacity-80 ${t.text.primary} rounded border ${t.border.input}`}
              >
                浏览...
              </button>
            </div>
          </div>

          {/* 每次录制时弹文件对话框 */}
          <div>
            <label className={`flex items-center gap-2 text-sm cursor-pointer ${t.text.primary}`}>
              <input
                type="checkbox"
                checked={config.prompt_save_dialog}
                onChange={(e) => setPromptDialog(e.target.checked)}
              />
              每次录制时弹文件对话框
            </label>
            <p className={`text-xs ${t.text.muted} mt-1 ml-6`}>
              关闭时直接保存到上方默认路径（自动加 capture-{`{port}`}-{`{时间}`}.txt 文件名）
            </p>
          </div>
        </div>

        <div className="flex justify-end gap-2 mt-6">
          <button
            type="button"
            onClick={onClose}
            className={`px-4 py-2 text-sm ${t.bg.tertiary} hover:opacity-80 ${t.text.primary} rounded`}
          >
            取消
          </button>
          <button
            type="button"
            onClick={handleSave}
            className="px-4 py-2 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
