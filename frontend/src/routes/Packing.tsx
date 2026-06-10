import * as m from "framer-motion/m";
import { useMemo } from "react";
import { LoaderIcon, XIcon } from "../components/pack/icons";
import { PackProgressBar } from "../components/pack/PackProgressBar";
import { PhaseBreakdown } from "../components/pack/PhaseBreakdown";
import { ProgressLog } from "../components/pack/ProgressLog";
import { fadeUp } from "../lib/motion";
import { usePackJobContext } from "../lib/pack-job-context";
import { progressFromEvents } from "../lib/pack-progress";
import { useApp, useLastStats, usePackEvents } from "../lib/store";
import { useKeyboardShortcuts } from "../lib/use-keyboard-shortcuts";

/** Full-screen progress theater. The ONLY moment subscribed to the
 * high-churn events slice. Esc cancels (unless a sheet is open — the
 * global Esc handler closes that first). */
export default function Packing() {
  const events = usePackEvents();
  const lastStats = useLastStats();
  const target = useApp((s) => s.options.target.value);
  const { cancel, isRunning } = usePackJobContext();
  const progress = useMemo(() => progressFromEvents(events), [events]);

  useKeyboardShortcuts({
    escape: () => {
      if (useApp.getState().activeSheet) return;
      if (isRunning) cancel();
    },
  });

  return (
    <m.div
      className="mx-auto w-full max-w-2xl space-y-6 px-6 pb-16 pt-12"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="text-center">
        <div className="flex items-center justify-center gap-2 text-xs font-semibold uppercase tracking-wider text-primary">
          <m.span
            aria-hidden="true"
            animate={{ rotate: 360 }}
            transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
          >
            <LoaderIcon size={13} />
          </m.span>
          Packing
        </div>
        <h1 className="mt-2 truncate font-mono text-sm text-zinc-400">
          {target}
        </h1>
      </div>

      <PackProgressBar value={progress} />
      <ProgressLog events={events} />
      {lastStats && <PhaseBreakdown stats={lastStats} />}

      <div className="flex justify-center">
        <m.button
          type="button"
          onClick={() => cancel()}
          title="Cancel pack (Esc)"
          className="flex items-center gap-2 rounded-xl border border-red-600/50 bg-red-950/40 px-5 py-3 text-sm font-semibold text-red-300 transition-colors hover:bg-red-900/40 hover:text-red-200"
          whileTap={{ scale: 0.97 }}
        >
          <XIcon size={15} />
          Cancel
        </m.button>
      </div>
    </m.div>
  );
}
