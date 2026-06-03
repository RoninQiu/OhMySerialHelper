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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
