import { useState } from "react";
import { Terminal } from "./components/Terminal";
import { SerialToolbar } from "./components/SerialToolbar";

type ViewMode = "text" | "hex";

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>("text");

  return (
    <div className="flex flex-col h-screen bg-gray-900 text-gray-100">
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-4 py-2 bg-gray-800 border-b border-gray-700">
        <h1 className="text-lg font-semibold">🚀 OhMySerial</h1>
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-2">
            <span className="text-sm">视图:</span>
            <select
              value={viewMode}
              onChange={(e) => setViewMode(e.target.value as ViewMode)}
              className="px-2 py-1 bg-gray-700 rounded text-white text-sm"
            >
              <option value="text">文本</option>
              <option value="hex">HEX</option>
            </select>
          </label>
        </div>
      </div>

      {/* 串口工具栏 */}
      <SerialToolbar />

      {/* 终端区域 */}
      <div className="flex-1 p-4">
        <div className="h-full bg-gray-800 rounded-lg overflow-hidden border border-gray-700">
          <Terminal viewMode={viewMode} encoding="utf8" />
        </div>
      </div>

      {/* 状态栏 */}
      <div className="flex items-center justify-between px-4 py-2 bg-gray-800 border-t border-gray-700 text-sm text-gray-400">
        <span>就绪</span>
        <span>OhMySerial v0.1.0</span>
      </div>
    </div>
  );
}

export default App;
