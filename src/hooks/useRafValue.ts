/**
 * 任意值的 rAF 节流 hook
 *
 * 用途：把高频变化的源值（如 60Hz rxBytes）映射到 ~10-15Hz 显示
 * 设计：源值通过 ref 持有最新值，每 N 帧（默认每 4 帧 ≈ 15Hz）触发 setState
 *
 * 为什么不用 setTimeout：rAF 与浏览器刷新同步，避免丢帧或重复
 * 为什么不动源：精度不能丢，关口时仍可查到精确值
 */
import { useEffect, useRef, useState } from "react";

/** 多少帧才更新一次显示（4 帧 ≈ 15Hz @ 60fps） */
const DEFAULT_FRAME_SKIP = 4;

/**
 * 纯函数：决定一帧后 displayed 是否要更新
 * - counter 累加，到达 frameSkip 时重置并返回新值
 * - 浅比较避免无意义 setState
 */
export function nextRafValue<T>(
  prev: T,
  latest: T,
  prevCounter: number,
  frameSkip: number,
): { value: T; counter: number } {
  const counter = prevCounter + 1;
  if (counter < frameSkip) {
    return { value: prev, counter };
  }
  const value = Object.is(prev, latest) ? prev : latest;
  return { value, counter: 0 };
}

export function useRafValue<T>(source: T, frameSkip: number = DEFAULT_FRAME_SKIP): T {
  const [displayed, setDisplayed] = useState<T>(source);
  const latest = useRef<T>(source);
  const counter = useRef(0);

  // 每次 source 变，把最新值塞 ref
  useEffect(() => {
    latest.current = source;
  }, [source]);

  // 启动 rAF 循环
  useEffect(() => {
    let cancelled = false;
    let rafId = 0;
    const tick = () => {
      if (cancelled) return;
      const { value, counter: c } = nextRafValue(
        displayed,
        latest.current,
        counter.current,
        frameSkip,
      );
      counter.current = c;
      if (!Object.is(value, displayed)) {
        setDisplayed(value);
      }
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => {
      cancelled = true;
      cancelAnimationFrame(rafId);
    };
  }, [frameSkip, displayed]);

  return displayed;
}
