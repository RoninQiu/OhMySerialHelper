/**
 * 预设命令面板：CRUD + localStorage 持久化 + 快速发送
 */
import { useState } from "react";
import { usePresetStore, PresetCommand } from "../stores/presetStore";
import { useSerialStore } from "../stores/serialStore";
import { hexToBytes } from "../utils/hex";
import { useThemeClasses } from "../hooks/useThemeClasses";

export function PresetPanel() {
  const { commands, addCommand, deleteCommand } = usePresetStore();
  const isOpen = useSerialStore((s) => s.isOpen);
  const sendData = useSerialStore((s) => s.sendData);
  const t = useThemeClasses();

  const [draftName, setDraftName] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [draftType, setDraftType] = useState<"text" | "hex">("text");
  const [error, setError] = useState<string | null>(null);

  const handleAdd = () => {
    if (!draftName.trim()) {
      setError("请输入命令名");
      return;
    }
    if (!draftContent.trim()) {
      setError("请输入内容");
      return;
    }
    try {
      if (draftType === "hex") {
        hexToBytes(draftContent); // 验证 HEX 合法
      }
    } catch (e) {
      setError(`HEX 无效: ${(e as Error).message}`);
      return;
    }
    addCommand({
      name: draftName,
      content: draftContent,
      type: draftType,
      priority: 50,
      enabled: true,
      intervalMs: 1000,
    });
    setDraftName("");
    setDraftContent("");
    setError(null);
  };

  const handleSendOne = async (cmd: PresetCommand) => {
    setError(null);
    try {
      let bytes: Uint8Array;
      if (cmd.type === "text") {
        bytes = new TextEncoder().encode(cmd.content);
      } else {
        bytes = hexToBytes(cmd.content);
      }
      await sendData(bytes);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message ?? String(e);
      setError(`${cmd.name}: ${msg}`);
    }
  };

  return (
    <div
      className={`flex flex-col h-full ${t.bg.secondary} border-t ${t.border.default} p-3 gap-2 overflow-hidden`}
    >
      <div className="flex items-center justify-between">
        <h3 className={`text-sm font-semibold ${t.text.secondary}`}>预设命令</h3>
        <span className={`text-xs ${t.text.muted}`}>{commands.length} 条</span>
      </div>

      {/* 列表 */}
      <div className="flex-1 overflow-y-auto space-y-1">
        {commands.length === 0 ? (
          <div className={`text-xs ${t.text.muted} text-center py-4`}>
            暂无预设，添加一条开始
          </div>
        ) : (
          commands.map((cmd) => (
            <div
              key={cmd.id}
              className={`flex items-center gap-2 px-2 py-1.5 ${t.bg.item} rounded border ${t.border.default}`}
            >
              <span
                className={
                  cmd.type === "hex"
                    ? "px-1.5 py-0.5 text-xs rounded bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-200"
                    : "px-1.5 py-0.5 text-xs rounded bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200"
                }
              >
                {cmd.type.toUpperCase()}
              </span>
              <span
                className={`flex-1 text-sm ${t.text.secondary} truncate`}
                title={cmd.content}
              >
                {cmd.name}
              </span>
              <button
                onClick={() => void handleSendOne(cmd)}
                disabled={!isOpen}
                className="px-2 py-0.5 text-xs bg-green-600 hover:bg-green-700 disabled:bg-gray-300 dark:disabled:bg-gray-600 disabled:opacity-50 text-white rounded"
              >
                发送
              </button>
              <button
                onClick={() => deleteCommand(cmd.id)}
                className="px-2 py-0.5 text-xs bg-red-600 hover:bg-red-700 text-white rounded"
                title="删除"
              >
                ✕
              </button>
            </div>
          ))
        )}
      </div>

      {/* 新增表单 */}
      <div className={`border-t ${t.border.default} pt-2 space-y-1`}>
        <div className="flex gap-1">
          <input
            value={draftName}
            onChange={(e) => setDraftName(e.target.value)}
            placeholder="名称"
            className={`flex-1 px-2 py-1 text-sm ${t.bg.primary} ${t.text.primary} rounded border ${t.border.input} focus:border-blue-500 focus:outline-none`}
          />
          <select
            value={draftType}
            onChange={(e) => setDraftType(e.target.value as "text" | "hex")}
            className={`px-2 py-1 text-sm ${t.bg.tertiary} ${t.text.primary} rounded`}
          >
            <option value="text">文本</option>
            <option value="hex">HEX</option>
          </select>
        </div>
        <textarea
          value={draftContent}
          onChange={(e) => setDraftContent(e.target.value)}
          placeholder={draftType === "text" ? "命令内容" : "HEX 字节"}
          rows={2}
          className={`w-full px-2 py-1 text-sm ${t.bg.primary} ${t.text.primary} rounded border ${t.border.input} focus:border-blue-500 focus:outline-none font-mono resize-none`}
        />
        {error && (
          <div className="text-xs text-red-600 dark:text-red-300 px-1">
            ⚠ {error}
          </div>
        )}
        <button
          onClick={handleAdd}
          className="w-full px-2 py-1 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded"
        >
          + 添加
        </button>
      </div>
    </div>
  );
}
