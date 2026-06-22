/**
 * recorderStore - 录制功能 session-only 状态
 *
 * v1.2.0：录制不持久化（用户每次主动开始/停止），但写入路径、字节数等
 * 状态在 session 内同步给 UI。
 *
 * 持久化的字段（default_capture_path / prompt_save_dialog）在 configStore
 * 不在这里。
 */

import { create } from "zustand";

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

  startRecording: (path: string) => Promise<void>;
  stopRecording: () => Promise<RecorderSummary | null>;
  setBytesWritten: (n: number) => void;
}

export const useRecorderStore = create<RecorderState>(() => ({
  // 占位字段：commit 3 实现
  isRecording: false,
  currentPath: null,
  bytesWritten: 0,
  startedAt: null,

  startRecording: async (_path: string) => {
    throw new Error("not implemented (commit 1 stub)");
  },

  stopRecording: async () => {
    throw new Error("not implemented (commit 1 stub)");
  },

  setBytesWritten: (_n: number) => {
    // commit 3: set({ bytesWritten: n })
  },
}));
