import type { Channel } from "@tauri-apps/api/core";
import type {
  AppError,
  PackOptions,
  PackResult,
  ProgressEvent,
} from "../bindings";
import * as bindings from "../bindings";
import { commands } from "../bindings";

export const api = bindings;
export type {
  PackFormat,
  PackOptions,
  PackResult,
  PackStats,
  PlanValidation,
  ProgressEvent,
  Settings,
} from "../bindings";

/**
 * Discriminated union mirroring the `tauri-specta`-generated `Result` shape
 * so callers can pattern-match without depending on the auto-generated file.
 * Re-exported from this module so app code only needs `from "@/lib/api"`.
 */
export type ApiResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: AppError };

// ── Request deduplication ───────────────────────────────────────────────────
//
// A 5-second window protects against accidental double-fires (double-click,
// React StrictMode dev double-effect, or two components mounting the same
// hook). Each in-flight call is registered in the map; identical calls
// within the window get the same Promise. The entry is cleared when the
// promise settles OR the window expires, whichever comes first — so a
// 30-second pack doesn't leak cache after success.
const inflight = new Map<string, Promise<unknown>>();
const DEDUP_WINDOW_MS = 5_000;

function dedup<T>(key: string, factory: () => Promise<T>): Promise<T> {
  const existing = inflight.get(key);
  if (existing) return existing as Promise<T>;
  const promise = factory();
  inflight.set(key, promise);
  const clear = () => {
    if (inflight.get(key) === promise) inflight.delete(key);
  };
  promise.then(clear, clear);
  // Belt-and-suspenders: if the promise hangs (it shouldn't, but Tauri IPC
  // can stall under load), clear the entry after the window expires anyway
  // so a subsequent call gets a fresh attempt.
  setTimeout(clear, DEDUP_WINDOW_MS);
  return promise;
}

/** Stable cache key for `packStart`: target + protocol + format are the
 * fields that can produce identical concurrent requests in practice. */
function packStartKey(opts: PackOptions): string {
  return `packStart:${opts.target.kind}:${opts.target.value}:${opts.protocolVersion}:${opts.format}`;
}

export function packStartApi(
  opts: PackOptions,
  onEvent: Channel<ProgressEvent>,
): Promise<ApiResult<string>> {
  return dedup(packStartKey(opts), () => commands.packStart(opts, onEvent));
}

export function packGetResultApi(
  jobId: string,
): Promise<ApiResult<PackResult>> {
  return dedup(`packGetResult:${jobId}`, () => commands.packGetResult(jobId));
}

export function savePackOutputApi(
  suggestedFilename: string,
  contents: string,
): Promise<ApiResult<string | null>> {
  // Save dialogs naturally serialize behind the OS file picker, so dedup
  // here is mostly defensive against double-clicks before the dialog opens.
  // We key on filename + a content fingerprint (first 64 chars) instead of
  // the full string to avoid hashing megabytes per call.
  const key = `savePackOutput:${suggestedFilename}:${contents.slice(0, 64)}:${contents.length}`;
  return dedup(key, () => commands.savePackOutput(suggestedFilename, contents));
}
