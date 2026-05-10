import type { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { ProgressEvent } from "../bindings";
import { commands } from "../bindings";
import { batchEvents, createPackProgressChannel } from "./events";
import { useApp } from "./store";

interface UsePackJobReturn {
  /** Start a pack run with the current options. No-op if a pack is already in flight. */
  run: () => Promise<void>;
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
 *  - `Channel<ProgressEvent>` is created lazily on first run() and reused
 *    across subsequent runs. Tauri's Channel registers an internal IPC
 *    handler; reassigning onmessage to a no-op does NOT unregister it.
 *    Reusing one channel across runs prevents the O(packs run) handler
 *    leak. The handler is reassigned each pack to capture a fresh closure.
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
 *  - Cleanup on unmount clears the flush timer, sets the channel handler
 *    to a no-op, and drops the ref so Tauri can GC the IPC entry.
 */
export function usePackJob(): UsePackJobReturn {
  const options = useApp((s) => s.options);
  const status = useApp((s) => s.status);
  const setJob = useApp((s) => s.setJob);
  const setResult = useApp((s) => s.setResult);
  const reset = useApp((s) => s.reset);
  const pushEventsBatched = useApp((s) => s.pushEventsBatched);
  const pushEventStable = useApp((s) => s.pushEvent);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const channelRef = useRef<Channel<ProgressEvent> | null>(null);
  const eventBufferRef = useRef<ProgressEvent[]>([]);
  const flushTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const isRunning = status === "running";
  const isRunningRef = useRef(isRunning);
  isRunningRef.current = isRunning;

  // Stash latest store actions in a ref so the channel handler closure
  // (installed once per run) doesn't capture stale references when a
  // re-render swaps the function identity. Zustand actions are stable,
  // but routing them through a ref future-proofs against Zustand v5
  // behaviours and keeps the handler self-contained.
  const actionsRef = useRef({ pushEventsBatched, pushEventStable, setResult });
  actionsRef.current = { pushEventsBatched, pushEventStable, setResult };

  function flushBuffer() {
    const buf = eventBufferRef.current;
    if (buf.length === 0) return;
    eventBufferRef.current = [];
    // batchEvents in the store layer covers the boundary case (last
    // store event + first buffered event both walking). We pre-collapse
    // here too so the store's batch is smaller.
    actionsRef.current.pushEventsBatched(batchEvents(buf));
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
    reset();

    if (channelRef.current === null) {
      channelRef.current = createPackProgressChannel(() => {});
    }
    const channel = channelRef.current;

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
    setJob(startRes.data);
  }

  return {
    run,
    errorMsg,
    dismissError: () => setErrorMsg(null),
    isRunning,
  };
}
