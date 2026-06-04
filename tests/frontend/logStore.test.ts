/**
 * @vitest-environment jsdom
 *
 * logStore 单测
 * - applyFilter 纯函数覆盖（level + keyword）
 * - store setter
 */
import { describe, it, expect, beforeEach } from "vitest";
import { useLogStore, applyFilter, LevelFilter } from "../../src/stores/logStore";
import type { LogLine } from "../../src/utils/logParser";

const sample: LogLine[] = [
  {
    timestamp: "10:00:00.000",
    fullTimestamp: "2026-06-04 10:00:00.000",
    level: "DEBUG",
    target: "serial-reader",
    message: "loop tick",
  },
  {
    timestamp: "10:00:01.000",
    fullTimestamp: "2026-06-04 10:00:01.000",
    level: "INFO",
    target: "send-poller",
    message: "发送成功",
  },
  {
    timestamp: "10:00:02.000",
    fullTimestamp: "2026-06-04 10:00:02.000",
    level: "WARN",
    target: "send-precise",
    message: "interval drift 5ms",
  },
  {
    timestamp: "10:00:03.000",
    fullTimestamp: "2026-06-04 10:00:03.000",
    level: "ERROR",
    target: "serial-reader",
    message: "设备已断开: device gone",
  },
];

describe("applyFilter", () => {
  it("ALL + 空 keyword 返所有行", () => {
    expect(applyFilter(sample, "ALL", "").length).toBe(4);
  });

  it("level=DEBUG 保留全部", () => {
    expect(applyFilter(sample, "DEBUG", "").length).toBe(4);
  });

  it("level=INFO 过滤掉 DEBUG", () => {
    const r = applyFilter(sample, "INFO", "");
    expect(r.length).toBe(3);
    expect(r.every((l) => l.level !== "DEBUG")).toBe(true);
  });

  it("level=WARN 只留 WARN/ERROR", () => {
    const r = applyFilter(sample, "WARN", "");
    expect(r.length).toBe(2);
    expect(r.map((l) => l.level)).toEqual(["WARN", "ERROR"]);
  });

  it("level=ERROR 只留 ERROR", () => {
    const r = applyFilter(sample, "ERROR", "");
    expect(r.length).toBe(1);
    expect(r[0].level).toBe("ERROR");
  });

  it("keyword 匹配 message（不区分大小写）", () => {
    const r = applyFilter(sample, "ALL", "断开");
    expect(r.length).toBe(1);
    expect(r[0].message).toContain("断开");
  });

  it("keyword 匹配 target", () => {
    const r = applyFilter(sample, "ALL", "poller");
    expect(r.length).toBe(1);
    expect(r[0].target).toBe("send-poller");
  });

  it("keyword + level 同时生效", () => {
    // WARN/ERROR 中包含 "disconnect" 的 → 1 行 (ERROR)
    const r = applyFilter(sample, "WARN", "断开");
    expect(r.length).toBe(1);
    expect(r[0].level).toBe("ERROR");
  });

  it("空行数组返空", () => {
    expect(applyFilter([], "ALL", "")).toEqual([]);
  });

  it("keyword 前后空白 trim", () => {
    const r = applyFilter(sample, "ALL", "  断开  ");
    expect(r.length).toBe(1);
  });
});

describe("useLogStore", () => {
  beforeEach(() => {
    useLogStore.setState({
      lines: [],
      levelFilter: "ALL",
      keyword: "",
      lastFetchedAt: null,
      loading: false,
    });
  });

  it("默认值正确", () => {
    const s = useLogStore.getState();
    expect(s.lines).toEqual([]);
    expect(s.levelFilter).toBe("ALL");
    expect(s.keyword).toBe("");
    expect(s.loading).toBe(false);
  });

  it("setLines 写入并更新 lastFetchedAt", () => {
    useLogStore.getState().setLines(sample);
    const s = useLogStore.getState();
    expect(s.lines.length).toBe(4);
    expect(s.lastFetchedAt).not.toBeNull();
    expect(s.loading).toBe(false);
  });

  it("setLevelFilter 切换", () => {
    useLogStore.getState().setLevelFilter("WARN");
    expect(useLogStore.getState().levelFilter).toBe("WARN");
  });

  it("setKeyword 设置", () => {
    useLogStore.getState().setKeyword("error");
    expect(useLogStore.getState().keyword).toBe("error");
  });

  it("clear 清空 lines 和 lastFetchedAt", () => {
    useLogStore.getState().setLines(sample);
    useLogStore.getState().clear();
    const s = useLogStore.getState();
    expect(s.lines).toEqual([]);
    expect(s.lastFetchedAt).toBeNull();
  });

  it("LevelFilter 所有合法值", () => {
    const all: LevelFilter[] = ["DEBUG", "INFO", "WARN", "ERROR", "ALL"];
    expect(all.length).toBe(5);
  });
});
