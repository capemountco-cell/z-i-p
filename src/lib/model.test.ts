import { describe, expect, it } from "vitest";

import {
  afterPasswordError,
  errorText,
  fileName,
  filterEntries,
  formatBytes,
  newJob,
  normaliseSelection,
  percent,
  phaseText,
  rememberPassword,
  selectionSize,
  tildify,
  timeAgo,
} from "./model";
import type { Entry } from "./native";

const entry = (path: string, size: number | null = 1, isDir = false): Entry => ({
  path,
  isDir,
  size,
  encrypted: false,
  modified: null,
});

describe("words", () => {
  it("formats sizes like Finder", () => {
    expect(formatBytes(1)).toBe("1 byte");
    expect(formatBytes(999)).toBe("999 bytes");
    expect(formatBytes(1_234_567)).toBe("1.2 MB");
    expect(formatBytes(340_000)).toBe("340 KB");
    expect(formatBytes(null)).toBe("—");
  });

  it("names and paths", () => {
    expect(fileName("/Users/ada/Downloads/Photos.zip")).toBe("Photos.zip");
    expect(tildify("/Users/ada/Downloads")).toBe("~/Downloads");
  });

  it("percent and time", () => {
    expect(percent(5, 0)).toBeNull();
    expect(percent(50, 200)).toBe(25);
    expect(timeAgo(1000, 1030)).toBe("just now");
    expect(timeAgo(0, 7200)).toBe("2 h ago");
  });

  it("explains a full disk with figures", () => {
    expect(errorText({ kind: "notEnoughSpace", needed: 5e9, available: 1e9 })).toContain("needs 5.0 GB, 1.0 GB free");
  });

  it("shows progress on a row", () => {
    const job = { ...newJob("/a/b.zip"), state: { phase: "extracting" as const, done: 1, total: 4, current: "x" } };
    expect(phaseText(job)).toBe("Unpacking… 25%");
  });
});

describe("passwords", () => {
  it("tries remembered passwords before asking", () => {
    let job = newJob("/a/locked.rar");
    job = afterPasswordError(job, { kind: "passwordRequired" }, ["one", "two"]);
    expect(job.password).toBe("one");
    expect(job.state.phase).toBe("waiting");
    job = afterPasswordError(job, { kind: "wrongPassword" }, ["one", "two"]);
    expect(job.password).toBe("two");
    job = afterPasswordError(job, { kind: "wrongPassword" }, ["one", "two"]);
    expect(job.state).toEqual({ phase: "needsPassword", wrong: false });
    job = afterPasswordError({ ...job, password: "typed", passwordTyped: true }, { kind: "wrongPassword" }, ["one", "two"]);
    expect(job.state).toEqual({ phase: "needsPassword", wrong: true });
  });

  it("asks straight away with nothing remembered", () => {
    const job = afterPasswordError(newJob("/a.zip"), { kind: "passwordRequired" }, []);
    expect(job.state).toEqual({ phase: "needsPassword", wrong: false });
  });

  it("remembers newest first without repeats", () => {
    expect(rememberPassword(["a", "b"], "b")).toEqual(["b", "a"]);
    expect(rememberPassword(["a"], null)).toEqual(["a"]);
  });
});

describe("choosing files", () => {
  const entries = [entry("a", null, true), entry("a/1.txt", 10), entry("a/2.txt", 20), entry("ab.txt", 5)];

  it("a chosen folder covers what is inside it", () => {
    expect(normaliseSelection(new Set(["a", "a/1.txt", "ab.txt"]))).toEqual(["a", "ab.txt"]);
  });

  it("adds up what will be unpacked", () => {
    expect(selectionSize(entries, ["a"])).toBe(30);
    expect(selectionSize(entries, ["ab.txt"])).toBe(5);
  });

  it("filters by name", () => {
    expect(filterEntries(entries, "2.TX").map((e) => e.path)).toEqual(["a/2.txt"]);
  });
});
