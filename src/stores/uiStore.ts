/**
 * UI 状态：主题、布局等用户偏好
 */
import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "dark" | "light" | "system";

interface UiState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  /** 计算当前实际应用的主题（考虑 system 跟随） */
  resolvedTheme: () => "dark" | "light";
}

/** 监听系统主题变化 */
function watchSystemTheme(callback: () => void): () => void {
  if (typeof window === "undefined" || !window.matchMedia) {
    return () => {};
  }
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  mq.addEventListener("change", callback);
  return () => mq.removeEventListener("change", callback);
}

/** 解析主题：system 返回实际系统主题 */
function resolveTheme(theme: Theme): "dark" | "light" {
  if (theme === "system") {
    if (typeof window !== "undefined" && window.matchMedia) {
      return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
    return "dark";
  }
  return theme;
}

/** 把主题应用到 document.documentElement.className */
function applyThemeClass(theme: Theme) {
  if (typeof document === "undefined") return;
  const resolved = resolveTheme(theme);
  const root = document.documentElement;
  if (resolved === "dark") {
    root.classList.add("dark");
    root.classList.remove("light");
  } else {
    root.classList.add("light");
    root.classList.remove("dark");
  }
}

export const useUiStore = create<UiState>()(
  persist(
    (set, get) => ({
      theme: "dark",
      setTheme: (theme) => {
        applyThemeClass(theme);
        set({ theme });
      },
      resolvedTheme: () => resolveTheme(get().theme),
    }),
    {
      name: "ui-prefs",
      version: 1,
      onRehydrateStorage: () => (state) => {
        if (state) {
          applyThemeClass(state.theme);
        }
      },
    },
  ),
);

// 启动时注册系统主题变化监听
if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
  watchSystemTheme(() => {
    const { theme } = useUiStore.getState();
    if (theme === "system") {
      applyThemeClass("system");
    }
  });
}
