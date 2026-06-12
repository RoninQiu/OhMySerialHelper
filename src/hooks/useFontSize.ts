/**
 * 同步 configStore.config.font_size 到 <html> style.fontSize
 * - 启动时由 main.tsx 显式同步调用一次（防 FOUC）
 * - 之后随 store 变化自动同步（React 组件中使用 useFontSize hook）
 */
import { useEffect } from "react";
import { useConfigStore, DEFAULT_CONFIG } from "../stores/configStore";

/** React hook：监听 store 变化 → 同步 <html> fontSize */
export function useFontSize(): void {
  const fontSize = useConfigStore((s) => s.config.font_size);

  useEffect(() => {
    document.documentElement.style.fontSize = `${fontSize}px`;
  }, [fontSize]);
}

/**
 * 同步应用字号（不进 React 生命周期，main.tsx 启动时同步调用）
 * 默认参数用 DEFAULT_CONFIG.font_size（14px），与 store 默认值保持一致。
 */
export function applyFontSizeSync(fontSize: number = DEFAULT_CONFIG.font_size): void {
  document.documentElement.style.fontSize = `${fontSize}px`;
}