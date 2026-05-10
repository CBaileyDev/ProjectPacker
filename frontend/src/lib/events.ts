import { Channel } from "@tauri-apps/api/core";
import type { ProgressEvent } from "./api";

export function createPackProgressChannel(
  onEvent: (e: ProgressEvent) => void,
): Channel<ProgressEvent> {
  const channel = new Channel<ProgressEvent>();
  channel.onmessage = onEvent;
  return channel;
}

/**
 * Collapse runs of consecutive `walking` events into the most recent one.
 *
 * The walker emits `walking { files_scanned: N }` once per scanned file, so a
 * 50k-file repo produces 50k near-identical progress ticks that the UI only
 * uses to render a counter. We keep the *latest* walking event in any run
 * (so the displayed file count stays accurate) and drop every preceding
 * walking event in that run. All non-walking events pass through untouched
 * — they each carry distinct UI meaning (started/done/error/etc.).
 *
 * Called from the 100ms event-batching buffer in `usePackJob` and the store's
 * `pushEventsBatched` so neither has to know the dedup rules.
 */
export function batchEvents(events: ProgressEvent[]): ProgressEvent[] {
  if (events.length === 0) return events;
  const out: ProgressEvent[] = [];
  for (let i = 0; i < events.length; i++) {
    const e = events[i];
    if (e.kind === "walking") {
      // Skip if the next event is also walking — it'll subsume this one.
      const next = events[i + 1];
      if (next && next.kind === "walking") continue;
    }
    out.push(e);
  }
  return out;
}
