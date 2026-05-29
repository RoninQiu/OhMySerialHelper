import { create } from "zustand";

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
  resetOverflow: () => void;
}

export const BUFFER_SIZES: BufferSize[] = [
  SIZE_1MB,
  SIZE_5MB,
  SIZE_10MB,
  SIZE_50MB,
];

export const useBufferStore = create<BufferState>((set) => ({
  bufferSize: SIZE_10MB,
  txBytes: 0,
  rxBytes: 0,
  overflowCount: 0,

  setBufferSize: (size) => set({ bufferSize: size }),
  incrementTx: (count) => set((s) => ({ txBytes: s.txBytes + count })),
  incrementRx: (count) => set((s) => ({ rxBytes: s.rxBytes + count })),
  resetOverflow: () => set({ overflowCount: 0 }),
}));
