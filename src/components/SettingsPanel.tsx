/**
 * SettingsPanel - 设置弹窗（v1.2.0）
 *
 * 设计：
 * - 照搬 HotkeyHelp 的 Modal 模式（fixed inset-0 z-50 bg-black/40）
 * - 字段：默认保存路径 + "每次录制时弹文件对话框" toggle
 * - 未来可扩展更多设置（HEX 分隔符、自动滚动等）
 *
 * 持久化：
 * - default_capture_path / prompt_save_dialog 走 useConfigStore → Rust cmd_save_config
 */

// 占位：commit 4 实现
export interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsPanel(_props: SettingsPanelProps) {
  return null;
}
