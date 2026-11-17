/**
 * The interface's state and the words it uses, kept free of React and of
 * Tauri so it can be tested on its own.
 */

import type { CoreError, Entry, Listing, Outcome } from "./native";

export type JobState =
  | { phase: "reading" }
  | { phase: "browsing" }
  | { phase: "waiting" }
  | { phase: "needsPassword"; wrong: boolean }
  | { phase: "extracting"; done: number; total: number; current: string }
  | { phase: "done"; outcome: Outcome }
  | { phase: "failed"; error: CoreError }
  | { phase: "cancelled" };

export interface Job {
  id: number;
  path: string;
  name: string;
  listing: Listing | null;
  password: string | null;
  /** How many remembered passwords have been tried on this archive. */
  triedRemembered: number;
  /** True when `password` came from someone typing it for this archive. */
  passwordTyped: boolean;
  only: string[] | null;
  destination: string | null;
  state: JobState;
}

let nextId = 1;

export function newJob(path: string): Job {
  return {
    id: nextId++,
    path,
    name: fileName(path),
    listing: null,
    password: null,
    triedRemembered: 0,
    passwordTyped: false,
    only: null,
    destination: null,
    state: { phase: "reading" },
  };
}

export function fileName(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function parentDir(path: string): string {
  const i = path.lastIndexOf("/");
  return i > 0 ? path.slice(0, i) : "/";
}

/** "~/Downloads" rather than "/Users/ada/Downloads". */
export function tildify(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}

/** Decimal units, as Finder shows them. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return "—";
  if (bytes < 1000) return `${bytes} ${bytes === 1 ? "byte" : "bytes"}`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = -1;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

export function percent(done: number, total: number): number | null {
  if (total <= 0) return null;
  return Math.min(100, Math.floor((done / total) * 100));
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n.toLocaleString()} ${n === 1 ? one : many}`;
}

export function timeAgo(seconds: number, now = Date.now() / 1000): string {
  const d = Math.max(0, now - seconds);
  if (d < 60) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  if (d < 86400 * 7) return `${Math.floor(d / 86400)} d ago`;
  return new Date(seconds * 1000).toLocaleDateString();
}

/** What went wrong, and what to do, in one or two short sentences. */
export function errorText(e: CoreError): string {
  switch (e.kind) {
    case "notFound":
      return "The archive is no longer where it was. It may have been moved or deleted.";
    case "unsupported":
      return `Can't open this: ${e.detail}`;
    case "passwordRequired":
      return "Needs a password.";
    case "wrongPassword":
      return "That password doesn't open it.";
    case "notEnoughSpace":
      return e.needed > 0
        ? `Not enough space: needs ${formatBytes(e.needed)}, ${formatBytes(e.available)} free. Nothing was written.`
        : "The disk filled up while unpacking. Nothing was kept; free some space and try again.";
    case "missingPart":
      return `A part is missing. ${e.detail}`;
    case "damaged":
      return `The archive is damaged or incomplete. ${e.detail}`.trim();
    case "cannotWrite":
      return `Can't write to ${tildify(e.path)}. Choose another folder in Settings.`;
    case "io":
      return e.detail;
    case "cancelled":
      return "Stopped. Nothing was kept.";
  }
}

/** The short status shown on a row. */
export function phaseText(job: Job): string {
  const s = job.state;
  switch (s.phase) {
    case "reading":
      return "Reading…";
    case "browsing":
      return job.listing ? `${plural(fileCount(job.listing.entries), "file")} · choose what to unpack` : "Ready";
    case "waiting":
      return "Waiting…";
    case "needsPassword":
      return s.wrong ? "Wrong password" : "Needs a password";
    case "extracting": {
      const p = percent(s.done, s.total);
      return p === null ? `Unpacking… ${formatBytes(s.done)}` : `Unpacking… ${p}%`;
    }
    case "done":
      return `${plural(s.outcome.files, "file")} · ${formatBytes(s.outcome.bytes)}`;
    case "failed":
      return errorText(s.error);
    case "cancelled":
      return "Stopped";
  }
}

export function fileCount(entries: Entry[]): number {
  return entries.filter((e) => !e.isDir).length;
}

export function isActive(job: Job): boolean {
  return ["reading", "waiting", "extracting"].includes(job.state.phase);
}

export function isFinished(job: Job): boolean {
  return ["done", "failed", "cancelled"].includes(job.state.phase);
}

/**
 * What to do after an extraction failed with a password problem: try the
 * next password remembered from this session, or ask the person.
 */
export function afterPasswordError(job: Job, error: CoreError, remembered: string[]): Job {
  const wrong = error.kind === "wrongPassword";
  if (!job.passwordTyped && job.triedRemembered < remembered.length) {
    return { ...job, password: remembered[job.triedRemembered] ?? null, triedRemembered: job.triedRemembered + 1, state: { phase: "waiting" } };
  }
  return { ...job, state: { phase: "needsPassword", wrong: wrong && job.passwordTyped } };
}

/** Remember a password that worked, newest first, without duplicates. */
export function rememberPassword(list: string[], password: string | null): string[] {
  if (!password) return list;
  return [password, ...list.filter((p) => p !== password)].slice(0, 10);
}

/** Entries matching a filter, in archive order. */
export function filterEntries(entries: Entry[], query: string): Entry[] {
  const q = query.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter((e) => e.path.toLowerCase().includes(q));
}

/** A selection reduced to what matters: a chosen folder covers its contents. */
export function normaliseSelection(selected: Set<string>): string[] {
  const all = [...selected].sort();
  return all.filter((p) => !all.some((other) => other !== p && p.startsWith(`${other}/`)));
}

/** Size of what a selection unpacks to. */
export function selectionSize(entries: Entry[], selected: string[]): number {
  return entries
    .filter((e) => !e.isDir && selected.some((s) => e.path === s || e.path.startsWith(`${s}/`)))
    .reduce((sum, e) => sum + (e.size ?? 0), 0);
}

export function depth(path: string): number {
  return path.split("/").length - 1;
}
