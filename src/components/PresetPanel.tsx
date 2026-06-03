/**
 * 预设命令面板：CRUD + localStorage 持久化 + 快速发送
 */
import { useState } from "react";
import { usePresetStore, PresetCommand } from "../stores/presetStore";
import { useSerialStore } from "../stores/serialStore";
import { hexToBytes } from "../utils/hex";

export function PresetPanel() {
  const { commands, addCommand, updateCommand, deleteCommand } =
    usePresetStore();
  const isOpen = useSerialStore((s) => s.isOpen);
  const sendData = useSerialStore((s) => s.sendData);

  const [editingId, setEditingId] = useState<string | null>(null);
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
    <div className="flex flex-col h-full bg-gray-800 border-t border-gray-700 p-3 gap-2 overflow-hidden">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-300">预设命令</h3>
        <span className="text-xs text-gray-500">{commands.length} 条</span>
      </div>

      {/* 列表 */}
      <div className="flex-1 overflow-y-auto space-y-1">
        {commands.length === 0 ? (
          <div className="text-xs text-gray-500 text-center py-4">
            暂无预设，添加一条开始
          </div>
        ) : (
          commands.map((cmd) => (
            <div
              key={cmd.id}
              className="flex items-center gap-2 px-2 py-1.5 bg-gray-900/50 rounded border border-gray-700"
            >
              <span
                className={`px-1.5 py-0.5 text-xs rounded ${
                  cmd.type === "hex" ? "bg-purple-900 text-purple-200" : "bg-blue-900 text-blue-200"
                }`}
              >
                {cmd.type.toUpperCase()}
              </span>
              <span className="flex-1 text-sm text-gray-200 truncate" title={cmd.content}>
                {cmd.name}
              </span>
              <button
                onClick={() => void handleSendOne(cmd)}
                disabled={!isOpen}
                className="px-2 py-0.5 text-xs bg-green-600 hover:bg-green-700 disabled:bg-gray-600 disabled:opacity-50 text-white rounded"
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
      <div className="border-t border-gray-700 pt-2 space-y-1">
        <div className="flex gap-1">
          <input
            value={draftName}
            onChange={(e) => setDraftName(e.target.value)}
            placeholder="名称"
            className="flex-1 px-2 py-1 text-sm bg-gray-900 text-gray-100 rounded border border-gray-700 focus:border-blue-500 focus:outline-none"
          />
          <select
            value={draftType}
            onChange={(e) => setDraftType(e.target.value as "text" | "hex")}
            className="px-2 py-1 text-sm bg-gray-700 text-white rounded"
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
          className="w-full px-2 py-1 text-sm bg-gray-900 text-gray-100 rounded border border-gray-700 focus:border-blue-500 focus:outline-none font-mono resize-none"
        />
        {error && (
          <div className="text-xs text-red-300 px-1">⚠ {error}</div>
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
