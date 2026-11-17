/**
 * Every call from the UI to Rust goes through this file, so the command
 * surface is visible in one place and argument shapes cannot drift.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getVersion } from "@tauri-apps/api/app";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

/** True inside the Tauri window; false in a plain browser tab. */
export function nativeAvailable(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!nativeAvailable()) {
    throw new Error("The native side is not running. Start the app with `npm run dev`.");
  }
  return invoke<T>(command, args);
}

// Shapes shared with crates/z-i-p-core (serde, camelCase).

export type Format = "zip" | "rar" | "sevenZ" | "tar" | "tarGz" | "tarBz2" | "tarXz" | "tarZst" | "gz" | "bz2" | "xz" | "zst";

export interface Entry {
  path: string;
  isDir: boolean;
  size: number | null;
  encrypted: boolean;
  modified: number | null;
}

export interface Listing {
  format: Format;
  formatLabel: string;
  stem: string;
  entries: Entry[];
  totalSize: number | null;
  archiveSize: number;
  encrypted: boolean;
  pieces: number;
  unsafeNames: number;
}

export interface Outcome {
  output: string;
  files: number;
  bytes: number;
  skipped: number;
  unsafeNames: number;
}

export type CoreError =
  | { kind: "notFound"; path: string }
  | { kind: "unsupported"; detail: string }
  | { kind: "passwordRequired" }
  | { kind: "wrongPassword" }
  | { kind: "notEnoughSpace"; needed: number; available: number }
  | { kind: "missingPart"; detail: string }
  | { kind: "damaged"; detail: string }
  | { kind: "cannotWrite"; path: string; detail: string }
  | { kind: "io"; detail: string }
  | { kind: "cancelled" };

export interface ExtractRequest {
  archive: string;
  destination: string;
  password?: string | null;
  only?: string[] | null;
  skipJunk: boolean;
  smartFolder: boolean;
  keepQuarantine: boolean;
}

export interface Settings {
  onOpen: "extract" | "browse";
  destination: "nextToArchive" | "ask" | "folder";
  folder: string | null;
  smartFolder: boolean;
  skipJunk: boolean;
  revealWhenDone: boolean;
  keepQuarantine: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  onOpen: "extract",
  destination: "nextToArchive",
  folder: null,
  smartFolder: true,
  skipJunk: true,
  revealWhenDone: true,
  keepQuarantine: true,
};

export interface HistoryItem {
  archive: string;
  output: string;
  at: number;
  files: number;
  bytes: number;
  error: string | null;
}

export interface ProgressEvent {
  job: number;
  done: number;
  total: number;
  current: string;
}

/** A thrown command error as a CoreError (strings become `io`). */
export function asCoreError(e: unknown): CoreError {
  if (e && typeof e === "object" && "kind" in e) return e as CoreError;
  return { kind: "io", detail: e instanceof Error ? e.message : String(e) };
}

export const inspect = (path: string, password?: string | null) => call<Listing>("inspect", { path, password: password ?? null });
export const extract = (job: number, request: ExtractRequest) => call<Outcome>("extract", { job, request });
export const cancel = (job: number) => call<boolean>("cancel", { job });
export const getSettings = () => call<Settings>("get_settings");
export const saveSettings = (settings: Settings) => call<void>("save_settings", { settings });
export const destinationFor = (archive: string) => call<string | null>("destination_for", { archive });
export const getHistory = () => call<HistoryItem[]>("get_history");
export const clearHistory = () => call<void>("clear_history");
export const reveal = (path: string) => call<void>("reveal", { path });
export const openPath = (path: string) => call<void>("open_path", { path });
export const takeOpened = () => call<string[]>("take_opened");
export const appVersion = () => (nativeAvailable() ? getVersion() : Promise.resolve("dev"));

/** Choose archives with the system open panel. */
export async function chooseArchives(): Promise<string[]> {
  if (!nativeAvailable()) return [];
  const picked = await openDialog({ multiple: true, directory: false, title: "Choose archives to unpack" });
  if (!picked) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/** Choose a folder; null if the person cancels. */
export async function chooseFolder(title: string, defaultPath?: string): Promise<string | null> {
  if (!nativeAvailable()) return null;
  const picked = await openDialog({ directory: true, multiple: false, title, defaultPath });
  return typeof picked === "string" ? picked : null;
}

export function onProgress(cb: (e: ProgressEvent) => void): Promise<UnlistenFn> {
  if (!nativeAvailable()) return Promise.resolve(() => {});
  return listen<ProgressEvent>("extract-progress", (e) => cb(e.payload));
}

/** Archives opened from Finder; the paths are collected with `takeOpened`. */
export function onOpened(cb: () => void): Promise<UnlistenFn> {
  if (!nativeAvailable()) return Promise.resolve(() => {});
  return listen("files-opened", () => cb());
}

export type DragState = { type: "over" } | { type: "leave" } | { type: "drop"; paths: string[] };

export function onDragDrop(cb: (e: DragState) => void): Promise<UnlistenFn> {
  if (!nativeAvailable()) return Promise.resolve(() => {});
  return getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "enter" || p.type === "over") cb({ type: "over" });
    else if (p.type === "drop") cb({ type: "drop", paths: p.paths });
    else cb({ type: "leave" });
  });
}
