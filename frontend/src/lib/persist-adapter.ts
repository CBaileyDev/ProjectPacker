import { LazyStore } from "@tauri-apps/plugin-store";
import type { StateStorage } from "zustand/middleware";

const STORE_FILE = "projectpacker.settings.json";
const DEBOUNCE_MS = 300;
const RETRY_BACKOFF_BASE_MS = 500;
const MAX_RETRIES = 3;

const ALLOWED_PROTOCOL_VERSIONS = ["grok-to-cc-v1"] as const;
const ALLOWED_FORMATS = ["xml", "markdown", "plainText"] as const;
const MIN_FILE_SIZE_KB = 1;
const MAX_FILE_SIZE_KB = 102_400;
const MAX_GOAL_LENGTH = 8192;

function clamp(n: number, min: number, max: number): number {
  if (!Number.isFinite(n)) return min;
  return Math.min(Math.max(Math.round(n), min), max);
}

/**
 * Validate + sanitize a serialized Zustand state blob in place. Returns the
 * possibly-modified JSON string ready to write to disk. Bad input gets
 * clamped to safe values rather than rejected — a corrupted setting should
 * heal itself on next save instead of bricking the persistence layer.
 *
 * Validations:
 *  - `options.maxFileSizeKb` clamped to `[1, 102_400]`.
 *  - `options.protocolVersion` whitelisted; reset to `'grok-to-cc-v1'`.
 *  - `options.format` whitelisted; reset to `'xml'`.
 *  - `options.goal` truncated to 8192 chars.
 *
 * Tolerates non-object / non-JSON input by returning it unchanged — the
 * Zustand layer is the source of truth for shape, this is just the moat.
 */
function sanitize(raw: string): string {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return raw;
  }
  if (!parsed || typeof parsed !== "object") return raw;

  const root = parsed as Record<string, unknown>;
  const state = root.state as Record<string, unknown> | undefined;
  const options = state?.options as Record<string, unknown> | undefined;
  if (!options) return raw;

  // maxFileSizeKb
  if (typeof options.maxFileSizeKb === "number") {
    options.maxFileSizeKb = clamp(
      options.maxFileSizeKb,
      MIN_FILE_SIZE_KB,
      MAX_FILE_SIZE_KB,
    );
  } else {
    options.maxFileSizeKb = 1024;
  }

  // protocolVersion
  if (
    typeof options.protocolVersion !== "string" ||
    !ALLOWED_PROTOCOL_VERSIONS.includes(
      options.protocolVersion as (typeof ALLOWED_PROTOCOL_VERSIONS)[number],
    )
  ) {
    options.protocolVersion = "grok-to-cc-v1";
  }

  // format
  if (
    typeof options.format !== "string" ||
    !ALLOWED_FORMATS.includes(
      options.format as (typeof ALLOWED_FORMATS)[number],
    )
  ) {
    options.format = "xml";
  }

  // goal length
  if (
    typeof options.goal === "string" &&
    options.goal.length > MAX_GOAL_LENGTH
  ) {
    options.goal = options.goal.slice(0, MAX_GOAL_LENGTH);
  }

  return JSON.stringify(parsed);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Zustand `StateStorage` adapter that proxies to Tauri's plugin-store with:
 *  - 300ms debounced writes (rapid typing → at most one disk write per 300ms),
 *  - JSON validation/clamping on every write (see `sanitize`),
 *  - 3-attempt exponential backoff (500/1000/2000ms) on transient failures.
 *
 * `flush()` immediately writes the pending value (e.g. before unload). Reads
 * always go straight through — no debounce — because reads are the cold
 * path on startup and stale-then-fresh behaviour would be confusing.
 */
class DebouncedTauriStorage implements StateStorage {
  private store = new LazyStore(STORE_FILE);
  private pending = new Map<string, string>();
  private flushTimer: ReturnType<typeof setTimeout> | null = null;

  async getItem(name: string): Promise<string | null> {
    // Honour a queued-but-not-yet-flushed write so the same render cycle
    // sees what it just wrote. Without this, a setItem→getItem within
    // 300ms returns the previous value.
    if (this.pending.has(name)) {
      return this.pending.get(name) ?? null;
    }
    const value = await this.store.get<string>(name);
    return value ?? null;
  }

  setItem(name: string, value: string): void {
    const safe = sanitize(value);
    this.pending.set(name, safe);
    if (this.flushTimer !== null) clearTimeout(this.flushTimer);
    this.flushTimer = setTimeout(() => {
      void this.flush();
    }, DEBOUNCE_MS);
  }

  async removeItem(name: string): Promise<void> {
    this.pending.delete(name);
    await this.writeWithRetry(async () => {
      await this.store.delete(name);
      await this.store.save();
    });
  }

  /** Synchronously cancel the debounce timer and write any pending values
   * to disk. Returns the underlying retry promise so callers can await it
   * (e.g. on app exit). */
  async flush(): Promise<void> {
    if (this.flushTimer !== null) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    if (this.pending.size === 0) return;
    const snapshot = new Map(this.pending);
    this.pending.clear();
    await this.writeWithRetry(async () => {
      for (const [name, value] of snapshot) {
        await this.store.set(name, value);
      }
      await this.store.save();
    });
  }

  /** Run `op` up to MAX_RETRIES times with exponential backoff. Each attempt
   * after the first waits 500ms × 2^(attempt-1). Final failure surfaces
   * via console.error rather than throwing — the persistence layer can't
   * meaningfully bubble up to the React tree. */
  private async writeWithRetry(op: () => Promise<void>): Promise<void> {
    let lastErr: unknown;
    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      try {
        await op();
        return;
      } catch (e) {
        lastErr = e;
        if (attempt < MAX_RETRIES - 1) {
          await sleep(RETRY_BACKOFF_BASE_MS * 2 ** attempt);
        }
      }
    }
    console.error("[persist-adapter] write failed after retries", lastErr);
  }
}

/** Singleton — Zustand expects a single storage instance per app so the
 * pending-write map and timer are shared across all setItem calls. */
export const tauriStoreAdapter = new DebouncedTauriStorage();
