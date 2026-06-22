/**
 * recorderStore - 录制功能 session-only 状态（v1.2.0）
 *
 * 设计：
 * - isRecording / currentPath / bytesWritten / startedAt：session 内同步给 UI
 * - 持久化字段（默认保存路径 / 弹对话框 toggle）在 configStore，不在这里
 * - startRecording: invoke cmd_start_recording（Rust 端创建文件 + 写文件头）
 * - stopRecording: invoke cmd_stop_recording（flush + 返回 summary）
 */

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface RecorderSummary {
  path: string;
  bytes_written: number;
  duration_ms: number;
}

interface RecorderState {
  isRecording: boolean;
  currentPath: string | null;
  bytesWritten: number;
  startedAt: number | null;

  startRecording: (
    path: string,
    portName: string,
    baudRate: number,
    dataBits: number,
    stopBits: number,
    parity: string,
  ) => Promise<void>;
  stopRecording: () => Promise<RecorderSummary | null>;
  setBytesWritten: (n: number) => void;
}

export const useRecorderStore = create<RecorderState>((set) => ({
  isRecording: false,
  currentPath: null,
  bytesWritten: 0,
  startedAt: null,

  startRecording: async (path, portName, baudRate, dataBits, stopBits, parity) => {
    if (useRecorderStore.getState().isRecording) {
      throw new Error("已在录制中");
    }
    await invoke("cmd_start_recording", {
      path,
      portName,
      baudRate,
      dataBits,
      stopBits,
      parity,
    });
    set({
      isRecording: true,
      currentPath: path,
      bytesWritten: 0,
      startedAt: Date.now(),
    });
  },

  stopRecording: async () => {
    try {
      const summary = await invoke<RecorderSummary>("cmd_stop_recording");
      set({
        isRecording: false,
        currentPath: null,
        bytesWritten: 0,
        startedAt: null,
      });
      return summary;
    } catch {
      // 未在录制
      set({
        isRecording: false,
        currentPath: null,
        bytesWritten: 0,
        startedAt: null,
      });
      return null;
    }
  },

  setBytesWritten: (n) => set({ bytesWritten: n }),
}));