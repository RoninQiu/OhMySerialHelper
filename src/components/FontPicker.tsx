/**
 * 字体 Combobox
 * - 触发按钮显示当前字体
 * - 打开：input + ul 列表
 * - 第 1 项固定"系统默认（当前）" + 分隔线 + 字体列表
 * - a11y：打开 focus input / 关闭焦点回触发按钮
 * - 键盘：↑↓ 移动高亮 / Enter 选中 / Esc 关闭
 * - IME 中文拼音期间不响应键盘导航（reviewer 友情提示 #5）
 */
import { useEffect, useRef, useState } from "react";
import { useConfigStore } from "../stores/configStore";
import { useFontStore } from "../stores/fontStore";
import { SYSTEM_DEFAULT_KEY } from "../utils/fonts";

export function FontPicker() {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const [highlight, setHighlight] = useState(0);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const fonts = useFontStore((s) => s.fonts);
  const fontFamily = useConfigStore((s) => s.config.font_family);
  const setFontFamily = useConfigStore((s) => s.setFontFamily);

  // 第 1 项"系统默认" + 字体列表
  const items = [
    { family: SYSTEM_DEFAULT_KEY, label: "系统默认" },
    ...fonts.map((f) => ({ family: f.family, label: f.family })),
  ];

  const filtered = filter
    ? items.filter((i) => i.label.toLowerCase().includes(filter.toLowerCase()))
    : items;

  // 重置高亮当 filter / open 变时
  useEffect(() => {
    setHighlight(0);
  }, [filter, open]);

  // 打开/关闭 a11y 焦点管理
  useEffect(() => {
    if (open) {
      const raf = requestAnimationFrame(() => inputRef.current?.focus());
      return () => cancelAnimationFrame(raf);
    } else if (triggerRef.current) {
      const raf = requestAnimationFrame(() => triggerRef.current?.focus());
      return () => cancelAnimationFrame(raf);
    }
  }, [open]);

  // 点击外部关闭
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  // 键盘交互：Esc / ↑ / ↓ / Enter（IME 拼音期间不响应）
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      // reviewer 友情提示 #5：中文输入法拼音期间不响应键盘导航
      if (e.isComposing) return;
      if (e.key === "Escape") {
        setOpen(false);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlight((h) => Math.min(h + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlight((h) => Math.max(h - 1, 0));
      } else if (e.key === "Enter" && filtered.length > 0) {
        e.preventDefault();
        const item = filtered[highlight];
        if (item) {
          setFontFamily(item.family);
          setOpen(false);
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, filtered, highlight, setFontFamily]);

  const displayLabel = fontFamily === SYSTEM_DEFAULT_KEY ? "系统默认" : fontFamily;

  return (
    <div ref={containerRef} className="relative inline-block">
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="px-2 py-1 border rounded text-sm bg-white dark:bg-gray-800 dark:border-gray-700 min-w-[140px] text-left"
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        字体: {displayLabel}
      </button>
      {open && (
        <div className="absolute top-full left-0 mt-1 z-50 bg-white dark:bg-gray-800 border dark:border-gray-700 rounded shadow-lg">
          <input
            ref={inputRef}
            type="text"
            value={filter}
            onChange={(e) => {
              // reviewer 友情提示 #5：中文输入法拼音期间不重置 highlight
              const native = e.nativeEvent as InputEvent;
              if (native.isComposing) {
                setFilter(e.target.value);
                return;
              }
              setFilter(e.target.value);
              setHighlight(0);
            }}
            placeholder="搜索字体..."
            className="block w-full px-2 py-1 border-b dark:border-gray-700 bg-transparent text-sm"
            role="textbox"
          />
          <ul role="listbox" className="max-h-60 overflow-y-auto text-sm">
            {filtered.length === 0 ? (
              <li className="px-2 py-1 text-gray-500">
                {fonts.length === 0 ? "未找到等宽字体" : "无匹配字体"}
              </li>
            ) : (
              filtered.map((item, idx) => (
                <li
                  key={item.family}
                  role="option"
                  aria-selected={item.family === fontFamily}
                  className={`px-2 py-1 cursor-pointer hover:bg-blue-100 dark:hover:bg-blue-900 ${
                    idx === highlight ? "bg-blue-50 dark:bg-blue-950" : ""
                  } ${item.family === fontFamily ? "font-semibold" : ""}`}
                  onClick={() => {
                    setFontFamily(item.family);
                    setOpen(false);
                  }}
                >
                  {item.label}
                </li>
              ))
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
