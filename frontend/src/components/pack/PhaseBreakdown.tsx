import { motion } from "framer-motion";
import { memo, useMemo } from "react";
import type { PackStats } from "../../bindings";
import {
  barHeightTransition,
  fadeUp,
  prefersReducedMotion,
  staggerContainer,
} from "../../lib/motion";

interface Phase {
  key: string;
  label: string;
  value: number;
  /** Bar fill color. Each phase gets a distinct hue so a user can
   * eyeball the distribution without reading labels. */
  cls: string;
}

interface PhaseBreakdownProps {
  stats: PackStats;
}

/**
 * Animated 5-phase bar chart (Walk / Process / Secret Scan / Tokenize /
 * Emit). Bars scale relative to the longest phase, with a 4% floor so a
 * tiny phase still renders as a visible nub. Zero-value phases display
 * an em-dash above the bar so the user can tell "this phase ran for 0ms"
 * apart from "this phase was skipped".
 *
 * Wrapped in `React.memo` — re-renders only when the `stats` reference
 * changes. The phase list and max-ms calculation are `useMemo`'d for
 * the same reason.
 */
function PhaseBreakdownInner({ stats }: PhaseBreakdownProps) {
  const phases = useMemo<Phase[]>(
    () => [
      {
        key: "walk",
        label: "Walk",
        value: stats.walkMs,
        cls: "bg-blue-500/60",
      },
      {
        key: "process",
        label: "Process",
        value: stats.processMs,
        cls: "bg-violet-500/60",
      },
      {
        key: "secret",
        label: "Secret Scan",
        value: stats.secretScanMs ?? 0,
        cls: "bg-amber-500/60",
      },
      {
        key: "tokenize",
        label: "Tokenize",
        value: stats.tokenizeMs ?? 0,
        cls: "bg-cyan-500/60",
      },
      {
        key: "emit",
        label: "Emit",
        value: stats.emitMs,
        cls: "bg-emerald-500/60",
      },
    ],
    [stats],
  );

  const maxMs = useMemo(
    // Min of 1 so divide-by-zero never bites; visually equivalent to
    // showing only minimum-height bars.
    () => Math.max(...phases.map((p) => p.value), 1),
    [phases],
  );

  return (
    <motion.div
      className="mt-3 space-y-1.5"
      variants={staggerContainer}
      initial="hidden"
      animate="visible"
    >
      <div className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-zinc-600">
        Phase Breakdown
      </div>
      <div className="flex gap-3">
        {phases.map((phase) => {
          // 4% floor: any non-zero phase reads as a nub, even if it
          // ran for 1ms next to a 30s walk.
          const pct = Math.max(4, (phase.value / maxMs) * 100);
          const isZero = phase.value === 0;
          return (
            <motion.div
              key={phase.key}
              className="flex-1"
              title={`${phase.label}: ${phase.value}ms`}
              variants={fadeUp}
            >
              <div className="mb-1 text-center text-[10px] font-mono text-zinc-500">
                {phase.value > 0 ? `${phase.value}ms` : "—"}
              </div>
              <div
                className="flex h-16 items-end gap-0.5"
                role="img"
                aria-label={`${phase.label}: ${phase.value}ms`}
              >
                <motion.div
                  className={`w-full rounded-t ${phase.cls}`}
                  initial={{ height: 0 }}
                  animate={{ height: isZero ? 4 : `${pct}%` }}
                  transition={barHeightTransition(prefersReducedMotion)}
                />
              </div>
              <div className="mt-1 text-center text-[10px] text-zinc-500">
                {phase.label}
              </div>
            </motion.div>
          );
        })}
      </div>
    </motion.div>
  );
}

export const PhaseBreakdown = memo(PhaseBreakdownInner);
