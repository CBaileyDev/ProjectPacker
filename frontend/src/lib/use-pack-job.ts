import type { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { ProgressEvent } from "../bindings";
import { commands } from "../bindings";
import { createPackProgressChannel } from "./events";
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

/**
 * Hook owning the pack-job lifecycle: Channel<ProgressEvent> reuse across
 * runs, isRunning ref for re-entry guards, errorMsg state, and the runPack
 * function that wires packStart → Channel handler → packGetResult →
 * setResult / setErrorMsg.
 *
 * The Channel is created lazily on first run() and reused across subsequent
 * runs. Tauri's Channel registers an internal IPC handler; reassigning
 * onmessage to a no-op does NOT unregister it. Reusing one channel across
 * runs prevents the O(packs run) handler leak. The handler is reassigned
 * each pack to capture a fresh closure (jobId, etc.), but the underlying
 * Tauri IPC subscription stays the same. Cleanup on unmount sets the
 * handler to a no-op and drops the reference so the Tauri side can GC.
 */
export function usePackJob(): UsePackJobReturn {
  const { options, status, setJob, pushEvent, setResult, reset } = useApp();
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const channelRef = useRef<Channel<ProgressEvent> | null>(null);
  const isRunning = status === "running";
  const isRunningRef = useRef(isRunning);
  isRunningRef.current = isRunning;

  useEffect(() => {
    return () => {
      if (channelRef.current) {
        channelRef.current.onmessage = () => {};
        channelRef.current = null;
      }
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

    // Install the real handler BEFORE awaiting packStart. Otherwise an
    // event emitted between packStart returning (server side) and the JS
    // continuation reassigning onmessage would be silently swallowed by
    // the no-op handler. The `Started` event carries `job_id`, so we can
    // capture it from inside the handler instead of waiting for
    // packStart's return value.
    let capturedJobId: string | null = null;
    channel.onmessage = (e) => {
      pushEvent(e);
      if (e.kind === "started") {
        capturedJobId = e.job_id;
      }
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
          if (r.status === "ok") setResult(r.data);
          else setErrorMsg(r.error.message);
        })();
      }
      if (e.kind === "error" && e.fatal) {
        setErrorMsg(e.message);
      }
    };

    const startRes = await commands.packStart(options, channel);
    if (startRes.status !== "ok") {
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
