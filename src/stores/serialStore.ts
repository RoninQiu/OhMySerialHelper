import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import { useBufferStore } from "./bufferStore";

/** 后端推送的自动重连进度 */
export interface ReconnectStatus {
  state: "started" | "attempt" | "succeeded" | "failed" | "cancelled";
  attempt: number;
  max_attempts: number;
  next_delay_ms: number;
  message: string;
}

interface SerialState {
  isOpen: boolean;
  disconnected: boolean;
  portName: string;
  baudRate: number;
  dataBits: 5 | 6 | 7 | 8;
  stopBits: 1 | 2;
  parity: "none" | "odd" | "even";
  dtr: boolean;
  rts: boolean;
  encoding: "utf8" | "gbk";

  /** 自动重连进度（null = 未在重连） */
  reconnect: ReconnectStatus | null;

  setBaudRate: (baudRate: number) => void;
  openPort: (
    portName: string,
    baudRate: number,
    dataBits?: 5 | 6 | 7 | 8,
    stopBits?: 1 | 2,
    parity?: "none" | "odd" | "even",
  ) => Promise<void>;
  closePort: () => Promise<void>;
  cancelReconnect: () => Promise<void>;
  setDtr: (enabled: boolean) => Promise<void>;
  setRts: (enabled: boolean) => Promise<void>;
  setEncoding: (encoding: "utf8" | "gbk") => void;
  setDisconnected: (disconnected: boolean) => void;
  setReconnect: (status: ReconnectStatus | null) => void;
  sendData: (data: Uint8Array) => Promise<void>;
  /**
   * 注册串口数据回调（由 App.tsx 在挂载时绑定到 Tauri Channel）
   * 设计：channel 在 App.tsx 创建并通过 invoke 参数传给 cmd_open_port
   *       onmessage 回调需要写终端 + 增 rxBytes，但 store 不持有 terminalRef
   *       所以把"写终端"的部分用回调注入，store 仍负责 rxBytes
   */
  setDataHandler: (handler: ((data: Uint8Array) => void) | null) => void;
  /** 当前数据回调（由 setDataHandler 注入） */
  onData: ((data: Uint8Array) => void) | null;
}

export const useSerialStore = create<SerialState>()(
  subscribeWithSelector((set) => ({
    isOpen: false,
    disconnected: false,
    portName: "",
    baudRate: 115200,
    setBaudRate: (baudRate: number) => set({ baudRate }),
    dataBits: 8,
    stopBits: 1,
    parity: "none",
    dtr: false,
    rts: false,
    encoding: "utf8",
    reconnect: null,

    openPort: async (
      portName: string,
      baudRate: number,
      dataBits: 5 | 6 | 7 | 8 = 8,
      stopBits: 1 | 2 = 1,
      parity: "none" | "odd" | "even" = "none",
    ) => {
      const { invoke, Channel } = await import("@tauri-apps/api/core");
      // 零拷贝 channel：Rust 端 channel.send(Vec<u8>) 直推，绕过 JSON 序列化
      const onData = new Channel<number[]>();
      onData.onmessage = (payload: number[]) => {
        if (payload && payload.length > 0) {
          const data = new Uint8Array(payload);
          const handler = useSerialStore.getState().onData;
          if (handler) handler(data);
          useBufferStore.getState().incrementRx(payload.length);
        }
      };
      await invoke("cmd_open_port", {
        portName,
        baudRate,
        dataBits,
        stopBits,
        parity,
        onData,
      });
      set({ isOpen: true, portName, baudRate, disconnected: false, reconnect: null });
    },

    closePort: async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("cmd_close_port");
      set({ isOpen: false, disconnected: false, reconnect: null });
    },

    cancelReconnect: async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        await invoke("cmd_cancel_reconnect");
      } catch (e) {
        console.warn("cmd_cancel_reconnect failed:", e);
      }
    },

    setDtr: async (enabled) => {
      set({ dtr: enabled });
    },

    setRts: async (enabled) => {
      set({ rts: enabled });
    },

    setEncoding: (encoding) => set({ encoding }),
    setDisconnected: (disconnected) => set({ disconnected }),
    setReconnect: (reconnect) => set({ reconnect }),
    onData: null,
    setDataHandler: (handler) => set({ onData: handler }),

    /**
     * 发送数据，自动统计 TX 字节
     * 乐观更新：先自增 txBytes，invoke 失败时回滚
     */
    sendData: async (data: Uint8Array) => {
      const n = data.length;
      if (n === 0) return;

      // 乐观更新
      useBufferStore.getState().incrementTx(n);

      const { invoke } = await import("@tauri-apps/api/core");
      try {
        await invoke("cmd_write_data", { data: Array.from(data) });
      } catch (err) {
        // 回滚
        useBufferStore.getState().incrementTx(-n);
        throw err;
      }
    },
  })),
);
