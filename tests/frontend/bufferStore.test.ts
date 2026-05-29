/**
 * @vitest-environment jsdom
 */
import { describe, it, expect } from "vitest";
import { useBufferStore, BUFFER_SIZES } from "../../src/stores/bufferStore";

const SIZE_1MB = 1024 * 1024;
const SIZE_5MB = 5 * 1024 * 1024;
const SIZE_10MB = 10 * 1024 * 1024;
const SIZE_50MB = 50 * 1024 * 1024;

describe("bufferStore", () => {
  it("initializes with default values", () => {
    const state = useBufferStore.getState();
    expect(state.bufferSize).toBe(SIZE_10MB);
    expect(state.txBytes).toBe(0);
    expect(state.rxBytes).toBe(0);
    expect(state.overflowCount).toBe(0);
  });

  it("sets buffer size", () => {
    const { setBufferSize } = useBufferStore.getState();
    setBufferSize(SIZE_5MB);
    expect(useBufferStore.getState().bufferSize).toBe(SIZE_5MB);
  });

  it("increments tx bytes", () => {
    const { incrementTx } = useBufferStore.getState();
    incrementTx(100);
    expect(useBufferStore.getState().txBytes).toBe(100);
  });

  it("increments rx bytes", () => {
    const { incrementRx } = useBufferStore.getState();
    incrementRx(50);
    expect(useBufferStore.getState().rxBytes).toBe(50);
  });

  it("resets overflow count", () => {
    const store = useBufferStore.getState();
    store.incrementRx(1000);
    store.resetOverflow();
    expect(useBufferStore.getState().overflowCount).toBe(0);
  });
});

describe("BUFFER_SIZES", () => {
  it("contains expected sizes", () => {
    expect(BUFFER_SIZES).toContain(SIZE_1MB);
    expect(BUFFER_SIZES).toContain(SIZE_5MB);
    expect(BUFFER_SIZES).toContain(SIZE_10MB);
    expect(BUFFER_SIZES).toContain(SIZE_50MB);
  });
});
