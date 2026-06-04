/**
 * 配置同步 hook
 *
 * 监听各 store 变化 → 更新 configStore → debounce 500ms 写盘
 * 启动时调用一次 loadFromBackend
 *
 * 设计：用 subscribeWithSelector 限定订阅字段，避免 rxBytes 60Hz 触发 sync
 * - serial: portName/baudRate/dataBits/stopBits/parity/encoding
 * - buffer: bufferSize（仅这一项需要持久化）
 * - ui: theme
 */
import { useEffect, useRef } from "react";
import { useConfigStore, DEFAULT_CONFIG } from "../stores/configStore";
import { useSerialStore } from "../stores/serialStore";
import { useBufferStore, BUFFER_SIZES } from "../stores/bufferStore";
import { useUiStore } from "../stores/uiStore";

/** debounce 写盘（避免每个按键都写） */
const SAVE_DEBOUNCE_MS = 500;

export function useConfigSync(): void {
  const saveTimer = useRef<number | null>(null);

  // 1) 启动加载
  useEffect(() => {
    void useConfigStore.getState().loadFromBackend();
  }, []);

  // 2) 订阅变化 → 同步到 configStore → debounce 写盘
  useEffect(() => {
    const scheduleSave = () => {
      if (saveTimer.current !== null) {
        clearTimeout(saveTimer.current);
      }
      saveTimer.current = window.setTimeout(() => {
        void useConfigStore.getState().save();
        saveTimer.current = null;
      }, SAVE_DEBOUNCE_MS);
    };

    // 同步：拉一次当前值到 configStore
    const sync = () => {
      const serial = useSerialStore.getState();
      const buf = useBufferStore.getState();
      const ui = useUiStore.getState();

      // 验证 buffer_size 在合法集合内
      const validBuffer =
        BUFFER_SIZES.includes(buf.bufferSize as (typeof BUFFER_SIZES)[number])
          ? buf.bufferSize
          : DEFAULT_CONFIG.buffer_size;

      useConfigStore.setState({
        config: {
          last_port: serial.portName || null,
          baud_rate: serial.baudRate,
          data_bits: serial.dataBits,
          stop_bits: serial.stopBits,
          parity: serial.parity,
          encoding: serial.encoding,
          theme: ui.theme,
          buffer_size: validBuffer,
          auto_reconnect: useConfigStore.getState().config.auto_reconnect,
          reconnect_max_attempts:
            useConfigStore.getState().config.reconnect_max_attempts,
        },
      });

      // 仅在 config 已加载后才触发写盘
      if (useConfigStore.getState().loaded) {
        scheduleSave();
      }
    };

    // 初次同步一次（不写盘，等 loadFromBackend 完成后再写）
    sync();

    // 用 selector 订阅：只有指定字段变化才触发
    const unsubSerial = useSerialStore.subscribe(
      (s) =>
        [
          s.portName,
          s.baudRate,
          s.dataBits,
          s.stopBits,
          s.parity,
          s.encoding,
        ] as const,
      () => {
        if (useConfigStore.getState().loaded) {
          sync();
          scheduleSave();
        }
      },
      {
        equalityFn: (a, b) =>
          a[0] === b[0] &&
          a[1] === b[1] &&
          a[2] === b[2] &&
          a[3] === b[3] &&
          a[4] === b[4] &&
          a[5] === b[5],
      },
    );

    const unsubBuffer = useBufferStore.subscribe(
      (s) => s.bufferSize,
      () => {
        if (useConfigStore.getState().loaded) {
          sync();
          scheduleSave();
        }
      },
    );

    const unsubUi = useUiStore.subscribe(
      (s) => s.theme,
      () => {
        if (useConfigStore.getState().loaded) {
          sync();
          scheduleSave();
        }
      },
    );

    return () => {
      unsubSerial();
      unsubBuffer();
      unsubUi();
      if (saveTimer.current !== null) {
        clearTimeout(saveTimer.current);
      }
    };
  }, []);
}
