import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { useUiStore } from "./stores/uiStore";
import "./index.css";

// 启动时应用持久化的主题（避免首次渲染闪烁）
const initialTheme = useUiStore.getState().theme;
if (initialTheme === "light" || (initialTheme === "system" && !window.matchMedia?.("(prefers-color-scheme: dark)").matches)) {
  document.documentElement.classList.add("light");
} else {
  document.documentElement.classList.add("dark");
}

// 字号只影响终端（xterm.options.fontSize），不动 <html> 字号 —— UI 始终用 Tailwind 默认
// 这样不管字号调多大都不会撑破标题栏 / 工具栏 / 状态栏布局

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
