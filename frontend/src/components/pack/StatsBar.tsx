import * as m from "framer-motion/m";
import { memo } from "react";
import type { PackStats } from "../../bindings";
import { fmtBytes, fmtNum } from "../../lib/format";
import {
  fadeUp,
  prefersReducedMotion,
  slideInUp,
  staggerContainer,
} from "../../lib/motion";
import { AlertIcon, ClockIcon, FileIcon, PackageIcon, ZapIcon } from "./icons";
import { SkeletonBlock } from "./Skeleton";

interface StatsBarProps {
  stats: PackStats | null;
  loading?: boolean;
}

/**
 * Three-state stats bar:
 *  - skeleton (loading=true OR stats===null with loading inferred): six
 *    SkeletonBlock placeholders mimicking the icon+label+value triples
 *  - empty (loading=false AND stats===null): compact "No stats yet"
 *  - content (stats!==null): full StatsBar with animated progress-bar
 *    backdrop + staggered entrance for each item
 *
 * Wrapped in `React.memo` so a parent re-render that doesn't change
 * `stats` doesn't kick off the staggered re-entrance again.
 */
function StatsBarInner({ stats, loading }: StatsBarProps) {
  if (loading || (stats === null && loading !== false)) {
    return (
      <m.div
        className="rounded-xl border border-zinc-700/80 bg-zinc-800/40 px-5 py-4 backdrop-blur-sm"
        variants={fadeUp}
        initial="hidden"
        animate="visible"
        aria-busy="true"
        aria-label="Loading pack statistics"
      >
        <div className="flex flex-wrap gap-x-6 gap-y-3 text-sm">
          {/* Six skeleton triples: icon + label + value */}
          {Array.from({ length: 4 }).map((_, i) => (
            <div
              // biome-ignore lint/suspicious/noArrayIndexKey: static placeholder list
              key={i}
              className="flex items-center gap-2"
            >
              <SkeletonBlock width={14} height={14} className="rounded-full" />
              <SkeletonBlock width={48} height={12} />
              <SkeletonBlock width={64} height={14} />
            </div>
          ))}
        </div>
      </m.div>
    );
  }

  if (stats === null) {
    return (
      <m.div
        className="rounded-xl border border-zinc-700/80 bg-zinc-800/40 px-5 py-4 text-sm text-zinc-500"
        variants={fadeUp}
        initial="hidden"
        animate="visible"
      >
        No stats yet
      </m.div>
    );
  }

  const filePct =
    stats.filesTotal > 0
      ? Math.round((stats.filesIncluded / stats.filesTotal) * 100)
      : 0;

  const items = [
    {
      icon: <PackageIcon size={14} className="text-emerald-400" />,
      label: "Files",
      value: `${fmtNum(stats.filesIncluded)} / ${fmtNum(stats.filesTotal)}`,
      highlight: `${filePct}%`,
    },
    {
      icon: <FileIcon size={14} className="text-zinc-400" />,
      label: "Skipped",
      value: fmtNum(stats.filesSkipped),
    },
    {
      icon: <ZapIcon size={14} className="text-amber-400" />,
      label: "Size",
      value: fmtBytes(stats.bytesTotal),
    },
    {
      icon: <ClockIcon size={14} className="text-blue-400" />,
      label: "Time",
      value: `${stats.durationMs}ms`,
    },
  ];

  return (
    <m.div
      className="relative overflow-hidden rounded-xl border border-zinc-700/80 bg-zinc-800/60 px-5 py-4 backdrop-blur-sm"
      variants={staggerContainer}
      initial="hidden"
      animate="visible"
      aria-label="Pack statistics"
    >
      <m.div
        // Decorative progress backdrop — width tracks filesIncluded /
        // filesTotal so the user gets a glanceable "how much got
        // packed" without reading the percent badge.
        aria-hidden="true"
        className="absolute inset-y-0 left-0 bg-emerald-500/5"
        initial={{ width: "0%" }}
        animate={{ width: `${filePct}%` }}
        transition={
          prefersReducedMotion
            ? { duration: 0 }
            : { duration: 0.8, ease: [0.22, 1, 0.36, 1], delay: 0.1 }
        }
      />
      <div className="relative flex flex-wrap gap-x-6 gap-y-3 text-sm">
        {items.map((item) => (
          <m.span
            key={item.label}
            className="flex items-center gap-2"
            variants={slideInUp}
          >
            {item.icon}
            <span className="text-zinc-500">{item.label}</span>
            <span className="font-semibold text-zinc-100 nums">
              {item.value}
            </span>
            {item.highlight && (
              <span className="rounded bg-emerald-500/10 px-1.5 py-0.5 text-[11px] font-medium text-emerald-400">
                {item.highlight}
              </span>
            )}
          </m.span>
        ))}

        {stats.tokensTotal != null && (
          <m.span className="flex items-center gap-2" variants={fadeUp}>
            <ZapIcon size={14} className="text-violet-400" />
            <span className="text-zinc-500">Tokens</span>
            <span className="font-semibold text-zinc-100 nums">
              {fmtNum(stats.tokensTotal)}
            </span>
          </m.span>
        )}

        {stats.secretsFound > 0 && (
          <m.span
            className="flex items-center gap-1.5 font-semibold text-amber-400"
            initial={prefersReducedMotion ? false : { opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={
              prefersReducedMotion
                ? { duration: 0 }
                : {
                    type: "spring",
                    stiffness: 400,
                    damping: 20,
                    delay: 0.3,
                  }
            }
          >
            <AlertIcon size={14} />
            {stats.secretsFound} secret{stats.secretsFound !== 1 ? "s" : ""}{" "}
            detected
          </m.span>
        )}
      </div>
    </m.div>
  );
}

export const StatsBar = memo(StatsBarInner);
