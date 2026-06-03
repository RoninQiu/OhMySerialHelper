/**
 * 发送面板：文本/HEX 输入、发送/清空、Enter 快捷键
 */
import { useState, useCallback, KeyboardEvent } from "react";
import { useSerialStore } from "../stores/serialStore";
import { hexToBytes } from "../utils/hex";

type SendMode = "text" | "hex";
const NEWLINE_OPTIONS = [
  { label: "无", value: "" },
  { label: "\\r", value: "\r" },
  { label: "\\n", value: "\n" },
  { label: "\\r\\n", value: "\r\n" },
];

export function SendPanel() {
  const [mode, setMode] = useState<SendMode>("text");
  const [content, setContent] = useState("");
  const [newline, setNewline] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  const isOpen = useSerialStore((s) => s.isOpen);
  const sendData = useSerialStore((s) => s.sendData);

  const handleSend = useCallback(async () => {
    if (!content.trim() && !newline) {
      setError("内容为空");
      return;
    }
    setError(null);
    setSending(true);

    try {
      let bytes: Uint8Array;
      if (mode === "text") {
        const text = content + newline;
        bytes = new TextEncoder().encode(text);
      } else {
        // HEX 模式
        try {
          bytes = hexToBytes(content);
        } catch (e) {
          throw new Error(`HEX 解析失败: ${(e as Error).message}`);
        }
        if (newline) {
          const nl = new TextEncoder().encode(newline);
          const combined = new Uint8Array(bytes.length + nl.length);
          combined.set(bytes);
          combined.set(nl, bytes.length);
          bytes = combined;
        }
      }

      await sendData(bytes);
      setError(null);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message ?? String(e);
      setError(msg);
    } finally {
      setSending(false);
    }
  }, [mode, content, newline, sendData]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter 发送；Ctrl+Enter 换行
      if (e.key === "Enter" && !e.ctrlKey && !e.shiftKey) {
        e.preventDefault();
        void handleSend();
      }
    },
    [handleSend],
  );

  const handleClear = useCallback(() => {
    setContent("");
    setError(null);
  }, []);

  return (
    <div className="flex flex-col h-full bg-gray-800 border-l border-gray-700 p-3 gap-2">
      {/* 模式 + 换行 */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="flex rounded overflow-hidden border border-gray-600">
          <button
            onClick={() => setMode("text")}
            className={`px-3 py-1 text-sm ${
              mode === "text" ? "bg-blue-500 text-white" : "bg-gray-700 text-gray-300"
            }`}
          >
            文本
          </button>
          <button
            onClick={() => setMode("hex")}
            className={`px-3 py-1 text-sm ${
              mode === "hex" ? "bg-blue-500 text-white" : "bg-gray-700 text-gray-300"
            }`}
          >
            HEX
          </button>
        </div>
        <select
          value={newline}
          onChange={(e) => setNewline(e.target.value)}
          className="px-2 py-1 text-sm bg-gray-700 text-white rounded"
          title="追加换行符"
        >
          {NEWLINE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              换行: {opt.label}
            </option>
          ))}
        </select>
        <span className="text-xs text-gray-500 ml-auto">
          {mode === "hex" ? `${content.replace(/\s+/g, "").length / 2} 字节` : `${[...content].length} 字符`}
        </span>
      </div>

      {/* 输入框 */}
      <textarea
        value={content}
        onChange={(e) => setContent(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={
          mode === "text"
            ? "输入要发送的文本，按 Enter 发送，Ctrl+Enter 换行"
            : "输入 HEX 字节，例如 31 32 33 或 313233"
        }
        className="flex-1 px-3 py-2 bg-gray-900 text-gray-100 rounded border border-gray-700 focus:border-blue-500 focus:outline-none font-mono text-sm resize-none"
        spellCheck={false}
      />

      {/* 错误提示 */}
      {error && (
        <div className="px-2 py-1 text-xs bg-red-900/50 text-red-200 rounded">
          ⚠ {error}
        </div>
      )}

      {/* 操作按钮 */}
      <div className="flex gap-2">
        <button
          onClick={handleSend}
          disabled={!isOpen || sending}
          className="flex-1 px-3 py-2 bg-blue-500 hover:bg-blue-600 disabled:bg-gray-600 disabled:opacity-50 text-white rounded font-medium text-sm transition-colors"
        >
          {sending ? "发送中..." : "发送"}
        </button>
        <button
          onClick={handleClear}
          className="px-3 py-2 bg-gray-700 hover:bg-gray-600 text-gray-100 rounded text-sm transition-colors"
        >
          清空
        </button>
      </div>

      {!isOpen && (
        <div className="text-xs text-gray-500 text-center">
          请先打开串口
        </div>
      )}
    </div>
  );
}
