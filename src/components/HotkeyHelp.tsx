/**
 * 快捷键帮助浮层
 *
 * 列出所有全局快捷键。按 F1 或 ? 切换显示。
 */
import { useEffect, useState } from "react";
import { useThemeClasses } from "../hooks/useThemeClasses";
import { formatHotkey, Hotkey } from "../hooks/useHotkeys";

interface HotkeyHelpProps {
  /** 当前已注册的所有快捷键（由父组件提供） */
  hotkeys: Hotkey[];
}

export function HotkeyHelp({ hotkeys }: HotkeyHelpProps) {
  const [open, setOpen] = useState(false);
  const t = useThemeClasses();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // F1 或 ?（不需要修饰键，且不在 input 中）
      if (e.key === "F1" || e.key === "?") {
        const target = e.target as HTMLElement | null;
        const isEditable =
          target?.tagName === "INPUT" ||
          target?.tagName === "TEXTAREA" ||
          target?.isContentEditable;
        if (isEditable) return;
        e.preventDefault();
        setOpen((v) => !v);
      }
      // Esc 关闭
      if (e.key === "Escape" && open) {
        e.preventDefault();
        setOpen(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open]);

  if (!open) return null;

  // 按 description 分类到不同组（v1.1.0：增加"显示"组）
  const groups: Array<{ title: string; match: (desc: string) => boolean }> = [
    { title: "通用", match: (d) => /清空|聚焦|主题|日志/.test(d) },
    { title: "显示", match: (d) => /字号/.test(d) },
  ];

  const visibleHotkeys = hotkeys.filter((hk) => hk.description);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={() => setOpen(false)}
    >
      <div
        className={`${t.bg.secondary} ${t.text.primary} rounded-lg shadow-2xl p-6 min-w-[420px] max-w-[600px] border ${t.border.default}`}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold mb-4">⌨️ 快捷键</h2>
        {groups.map((g) => {
          const keys = visibleHotkeys.filter((hk) => hk.description && g.match(hk.description));
          if (keys.length === 0) return null;
          return (
            <div key={g.title} className="mb-3 last:mb-0">
              <h3 className={`text-xs font-semibold uppercase mb-1 ${t.text.muted}`}>
                {g.title}
              </h3>
              <table className="w-full text-sm">
                <tbody>
                  {keys.map((hk, i) => (
                    <tr
                      key={`${g.title}-${i}`}
                      className="border-b border-gray-200 dark:border-gray-700 last:border-0"
                    >
                      <td className="py-2 pr-6 font-mono text-blue-600 dark:text-blue-400">
                        {formatHotkey(hk)}
                      </td>
                      <td className="py-2">{hk.description}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          );
        })}
        <div className={`mt-4 text-xs ${t.text.muted} text-center`}>
          按 Esc 或点击空白处关闭 · 按 ? 或 F1 重新打开
        </div>
      </div>
    </div>
  );
}
