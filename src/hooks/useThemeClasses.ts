/**
 * 主题 class 助手
 *
 * 把散落在组件里的"亮/暗"硬编码类名集中到语义化常量。
 * 用 Tailwind 的 `dark:` 前缀：默认（无前缀）是亮色，`dark:` 是暗色。
 *
 * 用法：
 *   const t = useThemeClasses();
 *   <div className={t.bg.primary}>…</div>
 */
import { useUiStore } from "../stores/uiStore";

export interface ThemeClasses {
  /** 容器背景：外层/根 */
  bg: {
    /** 主背景：root、整页 */
    primary: string;
    /** 次背景：卡片、面板 */
    secondary: string;
    /** 三级背景：输入框、下拉 */
    tertiary: string;
    /** 四级：内嵌条目（preset 列表项） */
    item: string;
  };
  /** 前景色 */
  text: {
    primary: string;
    secondary: string;
    muted: string;
    inverse: string; // 按钮上的白字
  };
  /** 边框 */
  border: {
    default: string;
    input: string;
  };
  /** 状态色 */
  status: {
    connected: string;
    disconnected: string;
    rx: string;
    tx: string;
    warning: string;
  };
}

/** 暗色 class 集合（暴露给测试和外部使用） */
export const DARK_CLASSES: ThemeClasses = {
  bg: {
    primary: "dark:bg-gray-900",
    secondary: "dark:bg-gray-800",
    tertiary: "dark:bg-gray-700",
    item: "dark:bg-gray-900/50",
  },
  text: {
    primary: "dark:text-gray-100",
    secondary: "dark:text-gray-200",
    muted: "dark:text-gray-400",
    inverse: "dark:text-white",
  },
  border: {
    default: "dark:border-gray-700",
    input: "dark:border-gray-600",
  },
  status: {
    connected: "dark:text-green-400",
    disconnected: "dark:text-red-400",
    rx: "dark:text-yellow-400",
    tx: "dark:text-blue-400",
    warning: "dark:text-red-400",
  },
};

/** 浅色 class 集合（暴露给测试和外部使用） */
export const LIGHT_CLASSES: ThemeClasses = {
  bg: {
    primary: "bg-white",
    secondary: "bg-gray-50",
    tertiary: "bg-gray-100",
    item: "bg-white",
  },
  text: {
    primary: "text-gray-900",
    secondary: "text-gray-700",
    muted: "text-gray-600",
    inverse: "text-white",
  },
  border: {
    default: "border-gray-200",
    input: "border-gray-300",
  },
  status: {
    connected: "text-green-600",
    disconnected: "text-red-600",
    rx: "text-yellow-600",
    tx: "text-blue-600",
    warning: "text-red-600",
  },
};

/**
 * 返回当前主题对应的语义化 class 集合
 *
 * 组件应这样用：
 *   <div className={`${t.bg.secondary} ${t.border.default}`}>
 *
 * 设计目标：让 dark 模式成为"叠加"而非"默认"，便于浅色先行的设计哲学。
 */
export function useThemeClasses(): ThemeClasses {
  const resolved = useUiStore((s) => s.resolvedTheme());
  return resolved === "dark" ? DARK_CLASSES : LIGHT_CLASSES;
}
