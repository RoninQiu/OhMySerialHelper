import { create } from "zustand";

interface SerialState {
  isOpen: boolean;
  portName: string;
  baudRate: number;
  dataBits: 5 | 6 | 7 | 8;
  stopBits: 1 | 2;
  parity: "none" | "odd" | "even";
  dtr: boolean;
  rts: boolean;
  encoding: "utf8" | "gbk";

  setBaudRate: (baudRate: number) => void;
  openPort: (
    portName: string,
    baudRate: number,
    dataBits?: 5 | 6 | 7 | 8,
    stopBits?: 1 | 2,
    parity?: "none" | "odd" | "even",
  ) => Promise<void>;
  closePort: () => Promise<void>;
  setDtr: (enabled: boolean) => Promise<void>;
  setRts: (enabled: boolean) => Promise<void>;
  setEncoding: (encoding: "utf8" | "gbk") => void;
}

export const useSerialStore = create<SerialState>((set) => ({
  isOpen: false,
  portName: "",
  baudRate: 115200,
  setBaudRate: (baudRate: number) => set({ baudRate }),
  dataBits: 8,
  stopBits: 1,
  parity: "none",
  dtr: false,
  rts: false,
  encoding: "utf8",

  openPort: async (
    portName: string,
    baudRate: number,
    dataBits: 5 | 6 | 7 | 8 = 8,
    stopBits: 1 | 2 = 1,
    parity: "none" | "odd" | "even" = "none",
  ) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("cmd_open_port", {
      portName,
      baudRate,
      dataBits,
      stopBits,
      parity,
    });
    set({ isOpen: true, portName, baudRate });
  },

  closePort: async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("cmd_close_port");
    set({ isOpen: false });
  },

  setDtr: async (enabled) => {
    set({ dtr: enabled });
  },

  setRts: async (enabled) => {
    set({ rts: enabled });
  },

  setEncoding: (encoding) => set({ encoding }),
}));
