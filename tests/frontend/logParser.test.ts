/**
 * @vitest-environment jsdom
 *
 * logParser 纯函数单测
 */
import { describe, it, expect } from "vitest";
import { parseLogLine, levelAtLeast } from "../../src/utils/logParser";

describe("parseLogLine", () => {
  it("标准行正确解析", () => {
    const r = parseLogLine(
      "[2026-06-04 14:23:15.123] [INFO] [serial-reader] 设备已断开",
    );
    expect(r).not.toBeNull();
    expect(r!.timestamp).toBe("14:23:15.123");
    expect(r!.fullTimestamp).toBe("2026-06-04 14:23:15.123");
    expect(r!.level).toBe("INFO");
    expect(r!.target).toBe("serial-reader");
    expect(r!.message).toBe("设备已断开");
  });

  it("ERROR 行解析", () => {
    const r = parseLogLine(
      "[2026-06-04 14:23:15.123] [ERROR] [send-poller] 写入失败",
    );
    expect(r?.level).toBe("ERROR");
    expect(r?.target).toBe("send-poller");
  });

  it("无 '[' 前缀返 null", () => {
    expect(parseLogLine("not a log line")).toBeNull();
  });

  it("空字符串返 null", () => {
    expect(parseLogLine("")).toBeNull();
  });

  it("缺 level 返 null", () => {
    expect(parseLogLine("[2026-06-04 14:23:15.123] no level here")).toBeNull();
  });

  it("缺 target 返 null", () => {
    expect(
      parseLogLine("[2026-06-04 14:23:15.123] [INFO] missing target"),
    ).toBeNull();
  });

  it("未知 level 返 null", () => {
    expect(
      parseLogLine("[2026-06-04 14:23:15.123] [TRACE] [foo] bar"),
    ).toBeNull();
  });

  it("message 内含方括号也能正确切", () => {
    const r = parseLogLine(
      "[2026-06-04 14:23:15.123] [ERROR] [foo] [nested] brackets in msg",
    );
    expect(r?.message).toBe("[nested] brackets in msg");
  });

  it("timestamp 不含日期时 timestamp 为空, fullTimestamp 含原值", () => {
    const r = parseLogLine("[14:23:15.123] [INFO] [a] x");
    expect(r?.timestamp).toBe("");
    expect(r?.fullTimestamp).toBe("14:23:15.123");
  });
});

describe("levelAtLeast", () => {
  it("ERROR >= WARN", () => {
    expect(levelAtLeast("ERROR", "WARN")).toBe(true);
  });

  it("WARN >= WARN", () => {
    expect(levelAtLeast("WARN", "WARN")).toBe(true);
  });

  it("ERROR >= DEBUG", () => {
    expect(levelAtLeast("ERROR", "DEBUG")).toBe(true);
  });

  it("INFO < WARN", () => {
    expect(levelAtLeast("INFO", "WARN")).toBe(false);
  });

  it("DEBUG < INFO", () => {
    expect(levelAtLeast("DEBUG", "INFO")).toBe(false);
  });

  it("大小写不敏感", () => {
    expect(levelAtLeast("error", "warn")).toBe(true);
    expect(levelAtLeast("Error", "warn")).toBe(true);
  });

  it("未知级别返 false", () => {
    expect(levelAtLeast("TRACE", "INFO")).toBe(false);
    expect(levelAtLeast("INFO", "TRACE")).toBe(false);
  });
});
