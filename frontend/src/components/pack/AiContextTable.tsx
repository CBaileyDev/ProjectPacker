import { motion } from "framer-motion";
import { memo } from "react";
import type { TokensPerModel } from "../../bindings";
import { AI_MODELS } from "../../lib/ai-models";
import { fmtNum } from "../../lib/format";
import { fadeUp, prefersReducedMotion } from "../../lib/motion";
import { SkeletonRow } from "./Skeleton";
import { CheckIcon, XIcon } from "./icons";

interface AiContextTableProps {
  tokensPerModel: TokensPerModel | null;
  loading?: boolean;
}

/**
 * AI context-window compatibility table.
 *
 *  - Renders one row per entry in `AI_MODELS` (one per supported
 *    tokenizer family).
 *  - Each row shows: model name + optional "approx" badge + animated
 *    progress bar (green <80%, amber 80-100%, red >100%) + token /
 *    context-limit numbers + fit/misfit indicator.
 *  - Three states:
 *      loading=true → skeleton with one SkeletonRow per AI_MODELS entry
 *      loading=false + tokensPerModel=null → "Enable Count tokens..."
 *      tokensPerModel != null → real content
 */
function AiContextTableInner({
  tokensPerModel,
  loading,
}: AiContextTableProps) {
  if (loading) {
    return (
      <motion.div
        className="overflow-hidden rounded-xl border border-zinc-700/80 bg-zinc-800/40"
        variants={fadeUp}
        initial="hidden"
        animate="visible"
        aria-busy="true"
        aria-label="Loading AI context window compatibility"
      >
        <div className="border-b border-zinc-700/60 bg-zinc-800/80 px-4 py-3">
          <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400">
            AI Context Window Compatibility
          </span>
        </div>
        <div className="divide-y divide-zinc-800/60 px-4">
          {AI_MODELS.map((m) => (
            <SkeletonRow key={m.name} />
          ))}
        </div>
      </motion.div>
    );
  }

  if (!tokensPerModel) {
    return (
      <motion.div
        className="rounded-xl border border-zinc-700/80 bg-zinc-800/40 p-5 text-sm text-zinc-400"
        variants={fadeUp}
        initial="hidden"
        animate="visible"
      >
        Enable "Count tokens" in options to see AI context-window
        compatibility.
      </motion.div>
    );
  }

  return (
    <motion.div
      className="overflow-hidden rounded-xl border border-zinc-700/80"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="border-b border-zinc-700/60 bg-zinc-800/80 px-4 py-3">
        <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400">
          AI Context Window Compatibility
        </span>
      </div>
      <div className="divide-y divide-zinc-800/60">
        {AI_MODELS.map((m, i) => {
          const tokens = tokensPerModel[m.tokenModel];
          const fits = tokens <= m.context;
          const pct = (tokens / m.context) * 100;
          // Green: comfortable fit. Amber: closing in (80-100%). Red:
          // overflow. Red only fires when we're _over_ the limit, since
          // exactly 100% still technically fits and doesn't need alarm.
          const barColor = fits
            ? pct > 80
              ? "bg-amber-500"
              : "bg-emerald-500"
            : "bg-red-500";

          return (
            <motion.div
              key={m.name}
              className="flex items-center gap-4 px-4 py-3"
              initial={
                prefersReducedMotion
                  ? false
                  : { opacity: 0, x: -12 }
              }
              animate={{ opacity: 1, x: 0 }}
              transition={
                prefersReducedMotion
                  ? { duration: 0 }
                  : {
                      duration: 0.3,
                      delay: 0.05 * i,
                      ease: [0.22, 1, 0.36, 1],
                    }
              }
            >
              <div className="w-40 shrink-0">
                <span className="text-sm text-zinc-200">{m.name}</span>
                {m.approx && (
                  <span className="ml-1.5 rounded bg-zinc-700/60 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-zinc-400">
                    approx
                  </span>
                )}
              </div>

              <div className="flex-1">
                <div
                  className="h-2 overflow-hidden rounded-full bg-zinc-700/50"
                  role="progressbar"
                  aria-valuenow={Math.round(Math.min(pct, 100))}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label={`${m.name} fill ${Math.round(pct)} percent`}
                >
                  <motion.div
                    className={`h-full rounded-full ${barColor}`}
                    initial={{ width: 0 }}
                    animate={{ width: `${Math.min(pct, 100)}%` }}
                    transition={
                      prefersReducedMotion
                        ? { duration: 0 }
                        : {
                            duration: 0.7,
                            ease: [0.22, 1, 0.36, 1],
                            delay: 0.1 + i * 0.04,
                          }
                    }
                  />
                </div>
              </div>

              <div className="w-28 shrink-0 text-right text-xs">
                <span className="font-medium text-zinc-200">
                  {fmtNum(tokens)}
                </span>
                <span className="text-zinc-600">
                  {" / "}
                  {fmtNum(m.context)}
                </span>
              </div>

              <div className="w-16 shrink-0 text-right">
                {fits ? (
                  <span className="inline-flex items-center gap-1 text-xs font-medium text-emerald-400">
                    <CheckIcon size={12} />
                    {Math.round(pct)}%
                  </span>
                ) : (
                  <span className="inline-flex items-center gap-1 text-xs font-medium text-red-400">
                    <XIcon size={12} />
                    {Math.round(pct)}%
                  </span>
                )}
              </div>
            </motion.div>
          );
        })}
      </div>
      <div className="border-t border-zinc-700/60 bg-zinc-800/40 px-4 py-2.5 text-[11px] leading-relaxed text-zinc-500">
        Rows marked "approx" use a proxy tokenizer (cl100k for Claude/Grok,
        cl100k×1.05 ceil for Gemini) since the authentic tokenizers are not
        public.
      </div>
    </motion.div>
  );
}

export const AiContextTable = memo(AiContextTableInner);
