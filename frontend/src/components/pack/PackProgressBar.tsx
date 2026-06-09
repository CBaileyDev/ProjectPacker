import * as m from "framer-motion/m";
import { memo } from "react";
import { easeOutCustom, prefersReducedMotion } from "../../lib/motion";

interface PackProgressBarProps {
  /** Overall progress, 0–100. See `lib/pack-progress.ts` for the phase
   * → range mapping that produces this value. */
  value: number;
}

/**
 * Slim determinate-ish progress bar shown above the progress log while a
 * pack runs. The fill animates toward the latest phase-mapped value;
 * reduced-motion users get instant width updates instead of tweens.
 */
function PackProgressBarInner({ value }: PackProgressBarProps) {
  const pct = Math.min(100, Math.max(0, value));
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pct)}
      aria-label="Overall pack progress"
      className="h-1.5 w-full overflow-hidden rounded-full bg-zinc-800/80 ring-1 ring-zinc-700/40"
    >
      <m.div
        className="h-full rounded-full bg-gradient-to-r from-emerald-600 to-emerald-400"
        initial={false}
        animate={{ width: `${pct}%` }}
        transition={
          prefersReducedMotion
            ? { duration: 0 }
            : { duration: 0.3, ease: easeOutCustom }
        }
      />
    </div>
  );
}

export const PackProgressBar = memo(PackProgressBarInner);
