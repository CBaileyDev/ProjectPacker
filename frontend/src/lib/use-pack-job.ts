import type { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { ProgressEvent } from "../bindings";
import { commands } from "../bindings";
import { createPackProgressChannel } from "./events";
import { useApp } from "./store";

interface UsePackJobReturn {
  /** Start a pack run with the current options. No-op if a pack is already in flight. */
  run: () => Promise<void>;
  /** Cancel the in-flight pack. No-op when nothing is running. User-initiated,
   * so it resolves to the `cancelled` status — never the error banner/toast. */
  cancel: () => Promise<void>;
  /** Most recent error message, or null. Cleared at the start of each new run. */
  errorMsg: string | null;
  /** Manually clear the error banner. */
  dismissError: () => void;
  /** True while a pack is in flight (status === "running"). */
  isRunning: boolean;
}

const FLUSH_INTERVAL_MS = 100;

/**
 * Hook owning the pack-job lifecycle:
 *
 *  - A FRESH `Channel<ProgressEvent>` is created per `run()`. Tauri 2's
 *    `Channel<T>` is single-use: it maintains an internal `nextMessageIndex`
 *    and calls `cleanupCallback()` (which `unregisterCallback`s the channel
 *    from `__TAURI_INTERNALS__`) when the Rust-side handle drops and emits
 *    its END marker. Reusing one channel across packs silently corrupts
 *    pack #2 — its messages arrive with index 0..N but the channel's
 *    `nextMessageIndex` is already past them, so they queue in
 *    `pendingMessages` and `onmessage` is never called. Symptom: pack #2's
 *    Done never reaches the handler and the UI stays at "Packing…" forever.
 *
 *  - 100ms event-batching buffer: incoming `ProgressEvent`s land in
 *    `eventBufferRef`, get collapsed via `batchEvents` (consecutive `walking`
 *    events drop all but the most recent), and the survivors are flushed
 *    into the store via `pushEventsBatched`. A 50k-file pack would otherwise
 *    cause 50k React re-renders; this caps it at ~10 flushes/second.
 *
 *  - Terminal events (`done`, `error`) bypass the buffer and flush
 *    immediately — those drive UI state transitions the user is waiting
 *    for, so an extra 100ms here is perceptible.
 *
 *  - Cleanup on unmount clears the flush timer and detaches the most
 *    recent channel's handler so stragglers don't write to dead React
 *    state. The Channel itself auto-unregisters via its own END handling.
 */
export function usePackJob(): UsePackJobReturn {
  const options = useApp((s) => s.options);
  const status = useApp((s) => s.status);
  const setJob = useApp((s) => s.setJob);
  const setResult = useApp((s) => s.setResult);
  const reset = useApp((s) => s.reset);
  const pushEventsBatched = useApp((s) => s.pushEventsBatched);
  const pushEventStable = useApp((s) => s.pushEvent);
  const markCancelled = useApp((s) => s.markCancelled);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const channelRef = useRef<Channel<ProgressEvent> | null>(null);
  const eventBufferRef = useRef<ProgressEvent[]>([]);
  const flushTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  /** Job id of the in-flight pack: set from the `started` event (and the
   * `packStart` return value as a belt-and-braces fallback), cleared on
   * each new run. `cancel()` needs it for `packCancel`. */
  const jobIdRef = useRef<string | null>(null);
  /** True once the user asked to cancel the current run. A terminal
   * `error` event arriving afterwards is the backend acknowledging the
   * cancellation — swallow it instead of surfacing an error banner. */
  const cancelRequestedRef = useRef(false);

  const isRunning = status === "running";
  const isRunningRef = useRef(isRunning);
  isRunningRef.current = isRunning;

  // Stash latest store actions in a ref so the channel handler closure
  // (installed once per run) doesn't capture stale references when a
  // re-render swaps the function identity. Zustand actions are stable,
  // but routing them through a ref future-proofs against Zustand v5
  // behaviours and keeps the handler self-contained.
  const actionsRef = useRef({
    pushEventsBatched,
    pushEventStable,
    setResult,
    markCancelled,
  });
  actionsRef.current = {
    pushEventsBatched,
    pushEventStable,
    setResult,
    markCancelled,
  };

  function flushBuffer() {
    const buf = eventBufferRef.current;
    if (buf.length === 0) return;
    eventBufferRef.current = [];
    // The store's `pushEventsBatched` runs `batchEvents` over the stitched
    // tail + batch, so the walking-collapse happens exactly once there —
    // no pre-collapse here.
    actionsRef.current.pushEventsBatched(buf);
  }

  function startFlushTimer() {
    if (flushTimerRef.current !== null) return;
    flushTimerRef.current = setInterval(flushBuffer, FLUSH_INTERVAL_MS);
  }

  function stopFlushTimer() {
    if (flushTimerRef.current !== null) {
      clearInterval(flushTimerRef.current);
      flushTimerRef.current = null;
    }
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: unmount-only cleanup; stopFlushTimer only touches refs and must not re-run per render
  useEffect(() => {
    return () => {
      stopFlushTimer();
      if (channelRef.current) {
        channelRef.current.onmessage = () => {};
        channelRef.current = null;
      }
      eventBufferRef.current = [];
    };
  }, []);

  async function run() {
    if (isRunningRef.current) return; // double-click / pre-await reentry guard
    setErrorMsg(null);
    cancelRequestedRef.current = false;
    jobIdRef.current = null;
    reset();

    // Detach the previous channel's handler so any stragglers (shouldn't
    // happen — pack #1 has already terminated by now — but defensively)
    // can't write to the store mid-run #2.
    if (channelRef.current !== null) {
      channelRef.current.onmessage = () => {};
    }
    // Fresh channel per run. Tauri 2 `Channel<T>` is single-use; see the
    // hook-level doc comment for the failure mode of reuse.
    const channel = createPackProgressChannel(() => {});
    channelRef.current = channel;

    // Install the real handler BEFORE awaiting packStart. An event emitted
    // between packStart returning (server side) and the JS continuation
    // reassigning onmessage would otherwise be silently swallowed. The
    // `Started` event carries `job_id`, so we capture it from inside the
    // handler instead of waiting for packStart's return value.
    let capturedJobId: string | null = null;

    channel.onmessage = (e) => {
      const isTerminal = e.kind === "done" || (e.kind === "error" && e.fatal);

      if (e.kind === "started") {
        capturedJobId = e.job_id;
        jobIdRef.current = e.job_id;
      }

      // User-initiated cancel: the backend acknowledges by erroring the
      // job. That's expected, not failure — drain the buffer, mark the
      // run cancelled, and surface no error banner/toast.
      if (e.kind === "error" && cancelRequestedRef.current) {
        flushBuffer();
        stopFlushTimer();
        actionsRef.current.markCancelled();
        return;
      }

      if (isTerminal) {
        // Drain whatever's buffered, THEN apply the terminal event so the
        // UI sees the buffered progress before the result/error lands.
        flushBuffer();
        actionsRef.current.pushEventStable(e);
        stopFlushTimer();

        if (e.kind === "done") {
          const id = capturedJobId;
          if (!id) {
            // Shouldn't happen — `started` always precedes `done`. Fall back
            // to a useful error rather than swallowing.
            setErrorMsg("internal: done without started");
            return;
          }
          (async () => {
            const r = await commands.packGetResult(id);
            if (r.status === "ok") actionsRef.current.setResult(r.data);
            else setErrorMsg(r.error.message);
          })();
        } else if (e.kind === "error") {
          setErrorMsg(e.message);
        }
      } else {
        eventBufferRef.current.push(e);
      }
    };

    startFlushTimer();

    // The auto-generated binding re-throws any `Error`-instance the IPC
    // layer rejects with (e.g. PackOptions deserialization failures from a
    // stale persisted store) instead of returning {status:"error",...}.
    // Without this catch, those become unhandled promise rejections inside
    // a button click handler — the UI shows no error and the pack appears
    // to do nothing.
    let startRes: Awaited<ReturnType<typeof commands.packStart>>;
    try {
      startRes = await commands.packStart(options, channel);
    } catch (e) {
      stopFlushTimer();
      setErrorMsg(e instanceof Error ? e.message : String(e));
      return;
    }
    if (startRes.status !== "ok") {
      stopFlushTimer();
      setErrorMsg(startRes.error.message);
      return;
    }
    jobIdRef.current ??= startRes.data;
    setJob(startRes.data);
  }

  async function cancel() {
    if (!isRunningRef.current || cancelRequestedRef.current) return;
    const id = jobIdRef.current ?? useApp.getState().jobId;
    if (!id) return;
    cancelRequestedRef.current = true;
    try {
      const res = await commands.packCancel(id);
      if (res.status !== "ok") {
        // Cancel itself failed — the pack is still running, so re-arm and
        // surface the failure (this one IS an error, not a cancellation).
        cancelRequestedRef.current = false;
        setErrorMsg(res.error.message);
        return;
      }
    } catch (e) {
      cancelRequestedRef.current = false;
      setErrorMsg(e instanceof Error ? e.message : String(e));
      return;
    }
    // Resolve the UI immediately rather than waiting on a terminal event
    // the backend may or may not emit post-cancel. If a `done` raced the
    // cancel and lands afterwards, the store's markCancelled no-ops on
    // non-running status and the result still wins.
    flushBuffer();
    stopFlushTimer();
    actionsRef.current.markCancelled();
  }

  return {
    run,
    cancel,
    errorMsg,
    dismissError: () => setErrorMsg(null),
    isRunning,
  };
}
