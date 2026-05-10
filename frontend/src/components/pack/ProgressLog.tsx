import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { memo, useEffect, useMemo, useRef } from "react";
import type { ProgressEvent } from "../../bindings";
import { prefersReducedMotion } from "../../lib/motion";

const STAGE_ICONS: Record<string, string> = {
  started: "▶",
  walking: "◌",
  tokenizing: "◎",
  secretScanning: "◈",
  compressing: "◐",
  buildingOutput: "◉",
  cloning: "⬇",
  done: "✓",
  error: "✗",
  secretHit: "⚠",
};

const STAGE_COLORS: Record<string, string> = {
  started: "text-emerald-400",
  walking: "text-blue-400",
  tokenizing: "text-violet-400",
  secretScanning: "text-amber-400",
  compressing: "text-cyan-400",
  buildingOutput: "text-emerald-400",
  cloning: "text-blue-400",
  done: "text-emerald-400",
  error: "text-red-400",
  secretHit: "text-amber-400",
  fileFoundBatch: "text-zinc-500",
  fileSkipped: "text-zinc-500",
};

/** Circular-buffer render cap. Anything older is silently dropped at
 * render time; the underlying store keeps 500 events for debug purposes. */
const RENDER_BUFFER_CAP = 200;
/** Window size: render only the last N items so a 200-event buffer stays
 * cheap. Anything older mounts as overflow text the user can scroll to. */
const WINDOW_SIZE = 50;
/** Only the last K items get framer-motion enter animations; everything
 * older is rendered as plain `<div>` so we don't pay layout-thrash cost
 * for events the user has already moved past. */
const ANIMATED_TAIL = 5;

interface RenderedLine {
  /** Stable identity within this batch. We append-only, so the original
   * insertion index in the rendered slice is monotonic. */
  id: number;
  kind: string;
  text: string;
}

function eventText(e: ProgressEvent): string | null {
  switch (e.kind) {
    case "started":
      return `Started ${e.target_label}`;
    case "walking":
      return `Walking… ${e.files_scanned} files scanned`;
    case "tokenizing":
      return `Tokenizing… ${e.progress_pct}%`;
    case "secretScanning":
      return `Secret scan… ${e.progress_pct}%`;
    case "compressing":
      return `Compressing… ${e.progress_pct}%`;
    case "buildingOutput":
      return "Building output…";
    case "cloning":
      return `Cloning repository… ${e.progress_pct}%`;
    case "secretHit":
      return `Secret found in ${e.path} (line ${e.line})`;
    case "done":
      return "Done";
    case "error":
      return `Error: ${e.message}`;
    default:
      return null;
  }
}

/**
 * Virtualized progress log:
 *  - Slices the events array to the last 200 (circular-buffer cap).
 *  - Within that, renders the last 50 items inside a max-height
 *    scrollable container.
 *  - Walking events are deduped at render time (consecutive walking
 *    events collapse into the most recent one), in addition to the
 *    pre-pass that `lib/events.ts::batchEvents` already applies in the
 *    store.
 *  - Only the trailing 5 items animate (framer-motion AnimatePresence).
 *    Older items render as plain `<div>` so a long log doesn't cost
 *    one motion-component per row.
 *  - Auto-scrolls to the bottom on new events.
 */
function ProgressLogInner({ events }: { events: ProgressEvent[] }) {
  const scrollRef = useRef<HTMLDivElement | null>(null);

  const lines = useMemo<RenderedLine[]>(() => {
    // Cap the working set first so large logs don't dominate the
    // walking-dedup loop below.
    const recent = events.slice(-RENDER_BUFFER_CAP);
    const out: RenderedLine[] = [];
    for (let i = 0; i < recent.length; i++) {
      const e = recent[i];
      // Walking-event dedup: if the next event is also walking, skip
      // this one — the next iteration will subsume it.
      if (e.kind === "walking") {
        const next = recent[i + 1];
        if (next && next.kind === "walking") continue;
      }
      const text = eventText(e);
      if (text === null) continue;
      out.push({ id: i, kind: e.kind, text });
    }
    return out;
  }, [events]);

  // Window: only the last 50 lines mount.
  const windowStart = Math.max(0, lines.length - WINDOW_SIZE);
  const windowed = lines.slice(windowStart);
  const tailStart = Math.max(0, windowed.length - ANIMATED_TAIL);
  const olderRows = windowed.slice(0, tailStart);
  const tailRows = windowed.slice(tailStart);

  // Auto-scroll to the bottom whenever new events arrive. Comparing the
  // length avoids re-scrolling when the parent re-renders for an
  // unrelated state change.
  const lineCountRef = useRef(0);
  useEffect(() => {
    if (lines.length === lineCountRef.current) return;
    lineCountRef.current = lines.length;
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [lines.length]);

  return (
    <m.div
      className="overflow-hidden rounded-xl border border-zinc-700/80 bg-zinc-900/80 backdrop-blur-sm"
      initial={
        prefersReducedMotion ? false : { opacity: 0, y: 12, scale: 0.98 }
      }
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={
        prefersReducedMotion
          ? { duration: 0 }
          : { duration: 0.35, ease: [0.22, 1, 0.36, 1] }
      }
    >
      <div className="border-b border-zinc-700/60 px-4 py-2.5">
        <div className="flex items-center gap-2">
          <m.div
            aria-hidden="true"
            className="h-2 w-2 rounded-full bg-emerald-400"
            animate={
              prefersReducedMotion ? { opacity: 1 } : { opacity: [1, 0.3, 1] }
            }
            transition={
              prefersReducedMotion
                ? { duration: 0 }
                : { duration: 1.5, repeat: Infinity, ease: "easeInOut" }
            }
          />
          <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400">
            Progress
          </span>
        </div>
      </div>
      <div
        ref={scrollRef}
        role="log"
        aria-live="polite"
        aria-relevant="additions"
        aria-label="Pack progress events"
        className="max-h-48 overflow-y-auto px-4 py-2"
      >
        {olderRows.map((l, i) => (
          // Static rows render as plain <div> — they aren't moving, no
          // need for the framer-motion overhead.
          <div
            // biome-ignore lint/suspicious/noArrayIndexKey: append-only window; index is identity
            key={`static-${windowStart + i}-${l.kind}`}
            className={`flex items-center gap-2 py-0.5 font-mono text-xs ${
              STAGE_COLORS[l.kind] ?? "text-zinc-400"
            }`}
          >
            <span className="w-4 shrink-0 text-center opacity-60">
              {STAGE_ICONS[l.kind] ?? "·"}
            </span>
            <span className="truncate">{l.text}</span>
          </div>
        ))}
        <AnimatePresence initial={false}>
          {tailRows.map((l, i) => (
            <m.div
              // Append-only log; index in the trailing window is a stable
              // identity for as long as a line is on screen.
              key={`tail-${windowStart + tailStart + i}-${l.kind}`}
              className={`flex items-center gap-2 py-0.5 font-mono text-xs ${
                STAGE_COLORS[l.kind] ?? "text-zinc-400"
              }`}
              initial={prefersReducedMotion ? false : { opacity: 0, x: -8 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0 }}
              transition={
                prefersReducedMotion
                  ? { duration: 0 }
                  : { duration: 0.2, ease: "easeOut" }
              }
            >
              <span className="w-4 shrink-0 text-center opacity-60">
                {STAGE_ICONS[l.kind] ?? "·"}
              </span>
              <span className="truncate">{l.text}</span>
            </m.div>
          ))}
        </AnimatePresence>
        {lines.length === 0 && (
          <div className="py-2 text-xs text-zinc-600 italic">
            Waiting to start…
          </div>
        )}
      </div>
    </m.div>
  );
}

export const ProgressLog = memo(ProgressLogInner);
