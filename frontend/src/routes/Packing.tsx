import * as m from "framer-motion/m";
import { useMemo } from "react";
import { LoaderIcon, XIcon } from "../components/pack/icons";
import { PackProgressBar } from "../components/pack/PackProgressBar";
import { ProgressLog } from "../components/pack/ProgressLog";
import { usePackJobContext } from "../lib/pack-job-context";
import { progressFromEvents } from "../lib/pack-progress";
import { useApp, usePackEvents } from "../lib/store";

/** Full-screen progress theater. The only moment that renders the live
 * event stream — the high-churn subscription stays out of every other
 * screen. Esc-to-cancel is owned by the Shell's global handler so it
 * can't race the sheet-closing Esc. */
export default function Packing() {
  const events = usePackEvents();
  const target = useApp((s) => s.options.target.value);
  const { cancel } = usePackJobContext();
  const progress = useMemo(() => progressFromEvents(events), [events]);

  return (
    // Plain div: MomentView's wrapper is the single entrance-animation
    // owner — a route-level fadeUp on top compounds into a double slide.
    <div className="mx-auto w-full max-w-2xl space-y-6 px-6 pb-16 pt-12">
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
    </div>
  );
}
