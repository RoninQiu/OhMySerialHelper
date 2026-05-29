import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface PresetCommand {
  id: string;
  name: string;
  content: string;
  type: "text" | "hex";
  priority: number; // 1-100
  enabled: boolean;
  intervalMs: number;
}

interface PresetState {
  commands: PresetCommand[];
  isPolling: boolean;

  addCommand: (cmd: Omit<PresetCommand, "id">) => void;
  updateCommand: (id: string, updates: Partial<PresetCommand>) => void;
  deleteCommand: (id: string) => void;
  startPolling: () => void;
  stopPolling: () => void;
}

export const usePresetStore = create<PresetState>()(
  persist(
    (set) => ({
      commands: [],
      isPolling: false,

      addCommand: (cmd) =>
        set((s) => ({
          commands: [...s.commands, { ...cmd, id: crypto.randomUUID() }],
        })),

      updateCommand: (id, updates) =>
        set((s) => ({
          commands: s.commands.map((c) =>
            c.id === id ? { ...c, ...updates } : c,
          ),
        })),

      deleteCommand: (id) =>
        set((s) => ({
          commands: s.commands.filter((c) => c.id !== id),
        })),

      startPolling: () => set({ isPolling: true }),
      stopPolling: () => set({ isPolling: false }),
    }),
    { name: "presets" },
  ),
);
