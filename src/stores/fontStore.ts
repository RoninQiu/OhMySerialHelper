/**
 * 字体列表缓存（不持久化，OS 级数据）
 * 启动时调 cmd_list_fonts 一次，结果缓存到内存
 */
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface FontInfo {
  family: string;
}

interface FontState {
  fonts: FontInfo[];
  loaded: boolean;
  loadFonts: () => Promise<void>;
}

export const useFontStore = create<FontState>((set, get) => ({
  fonts: [],
  loaded: false,
  loadFonts: async () => {
    if (get().loaded) return; // 防御：避免重复 invoke
    try {
      const fonts = await invoke<FontInfo[]>("cmd_list_fonts");
      set({ fonts, loaded: true });
    } catch (e) {
      console.warn("[fontStore] 加载字体列表失败：", e);
      set({ fonts: [], loaded: true }); // loaded=true 避免反复重试
    }
  },
}));