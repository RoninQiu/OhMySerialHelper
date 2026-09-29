import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";

// Buffer size options in bytes
const SIZE_1MB = 1024 * 1024;
const SIZE_5MB = 5 * 1024 * 1024;
const SIZE_10MB = 10 * 1024 * 1024;
const SIZE_50MB = 50 * 1024 * 1024;

type BufferSize =
  | typeof SIZE_1MB
  | typeof SIZE_5MB
  | typeof SIZE_10MB
  | typeof SIZE_50MB;

interface BufferState {
  bufferSize: BufferSize;
  txBytes: number;
  rxBytes: number;
  overflowCount: number;

  setBufferSize: (size: BufferSize) => void;
  incrementTx: (count: number) => void;
  incrementRx: (count: number) => void;
  /**
   * 定时发送（队列轮询 / 周期发送）的字节数。
   *
   * 这两条路径由 Rust 直接写串口，前端既不调 sendData 也没有返回值，
   * 只能靠 `tx-echo` 事件补计数。与手动发送的计数天然不相交。
   */
  noteTimedSend: (count: number) => void;
  resetOverflow: () => void;
}

export const BUFFER_SIZES: BufferSize[] = [
  SIZE_1MB,
  SIZE_5MB,
  SIZE_10MB,
  SIZE_50MB,
];

export const useBufferStore = create<BufferState>()(
  subscribeWithSelector((set) => ({
    bufferSize: SIZE_10MB,
    txBytes: 0,
    rxBytes: 0,
    overflowCount: 0,

    setBufferSize: (size) => set({ bufferSize: size }),
    incrementTx: (count) => set((s) => ({ txBytes: s.txBytes + count })),
    incrementRx: (count) => set((s) => ({ rxBytes: s.rxBytes + count })),
    noteTimedSend: (count) => set((s) => ({ txBytes: s.txBytes + count })),
    resetOverflow: () => set({ overflowCount: 0 }),
  })),
);
