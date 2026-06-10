import * as m from "framer-motion/m";
import { useState } from "react";
import { SectionTitle } from "../components/home/SectionTitle";
import { CompressionPanel } from "../components/pack/CompressionPanel";
import { CopyButton } from "../components/pack/CopyButton";
import {
  AlertIcon,
  BridgeIcon,
  CheckIcon,
  FileTextIcon,
  PackageIcon,
} from "../components/pack/icons";
import { PhaseBreakdown } from "../components/pack/PhaseBreakdown";
import { SaveButton } from "../components/pack/SaveButton";
import { FitsCard } from "../components/results/FitsCard";
import { fmtBytes, fmtNum } from "../lib/format";
import { prefersReducedMotion, springButton, springQuick } from "../lib/motion";
import { COPY_BUTTON_LABELS, SAVE_FILENAMES } from "../lib/pack-meta";
import { clampList } from "../lib/paginate";
import { useApp, usePackOptions, usePackProgress } from "../lib/store";

export default function Results() {
  const { options } = usePackOptions();
  const { result } = usePackProgress();
  const reset = useApp((s) => s.reset);
  const setMoment = useApp((s) => s.setMoment);
  const [showAllWarnings, setShowAllWarnings] = useState(false);
  const [showAllRedactions, setShowAllRedactions] = useState(false);

  if (!result) {
    return (
      <div className="mx-auto flex min-h-[60vh] w-full max-w-2xl flex-col items-center justify-center px-6 text-center">
        <FileTextIcon size={28} className="text-zinc-500" />
        <h3 className="mt-4 text-base font-semibold text-zinc-200">
          No pack results yet
        </h3>
        <m.button
          type="button"
          className="mt-5 flex items-center gap-2 rounded-lg bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground"
          onClick={() => setMoment("home")}
          whileTap={springButton}
        >
          <PackageIcon size={14} />
          Back to Home
        </m.button>
      </div>
    );
  }

  const previewLimit = 8000;
  const preview = result.output.slice(0, previewLimit);
  const truncated = result.output.length > previewLimit;
  const warnings = clampList(result.warnings, showAllWarnings);
  const redactions = clampList(result.redactions, showAllRedactions);
  const stats = result.stats;

  return (
    // Plain div: MomentView's wrapper is the single entrance-animation
    // owner — a route-level fadeUp on top compounds into a double slide.
    <div className="mx-auto w-full max-w-3xl space-y-6 px-6 pb-16 pt-8">
      {/* Crumb */}
      <div className="flex items-center gap-2 font-mono text-xs text-zinc-500">
        <span className="truncate">{options.target.value}</span>
        <span aria-hidden="true">·</span>
        <span>{options.format}</span>
        <span aria-hidden="true">·</span>
        <span>{fmtNum(stats.durationMs)} ms</span>
        <button
          type="button"
          onClick={() => reset()}
          className="ml-auto shrink-0 text-zinc-400 underline-offset-2 transition-colors hover:text-zinc-100 hover:underline"
        >
          ← new pack
        </button>
      </div>

      {/* Hero */}
      <section className="space-y-1.5">
        <div className="flex items-center gap-2">
          <m.span
            aria-hidden="true"
            className="flex h-5 w-5 items-center justify-center rounded-full bg-success/15 text-success ring-1 ring-success/25"
            initial={prefersReducedMotion ? false : { scale: 0, rotate: -90 }}
            animate={{ scale: 1, rotate: 0 }}
            transition={
              prefersReducedMotion
                ? { duration: 0 }
                : { ...springQuick, delay: 0.15 }
            }
          >
            <CheckIcon size={12} strokeWidth={2} />
          </m.span>
          <span className="text-xs font-semibold uppercase tracking-wider text-success">
            Pack complete
          </span>
        </div>
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <span className="nums text-4xl font-bold tracking-tight text-primary">
            {stats.tokensTotal != null
              ? fmtNum(stats.tokensTotal)
              : fmtNum(stats.filesIncluded)}
          </span>
          <span className="text-sm text-zinc-400">
            {stats.tokensTotal != null ? "tokens · " : ""}
            {fmtNum(stats.filesIncluded)} files · {fmtBytes(stats.bytesTotal)}
            {stats.secretsFound > 0
              ? ` · ${fmtNum(stats.secretsFound)} secrets redacted`
              : ""}
          </span>
        </div>
      </section>

      {/* Actions */}
      <section className="flex flex-wrap gap-2.5">
        <CopyButton
          label={COPY_BUTTON_LABELS[options.format]}
          text={result.output}
        />
        <SaveButton
          label="Save to file…"
          suggestedFilename={SAVE_FILENAMES[options.format]}
          text={result.output}
        />
        <CopyButton
          label="Copy Executor Prompt"
          text={result.executorPrompt}
          shortcut={false}
        />
      </section>

      <FitsCard tokensPerModel={stats.tokensPerModel} />
      <PhaseBreakdown stats={stats} />
      <CompressionPanel />

      {/* Bridge banner */}
      <button
        type="button"
        onClick={() => setMoment("bridge")}
        className="flex w-full items-center justify-between gap-4 rounded-xl border border-primary/30 bg-gradient-to-r from-primary/10 to-transparent px-5 py-4 text-left transition-colors hover:border-primary/50"
      >
        <span>
          <span className="flex items-center gap-2 text-sm font-semibold text-zinc-100">
            <BridgeIcon size={15} className="text-primary" />
            Got a plan back from your planner?
          </span>
          <span className="mt-1 block text-xs text-zinc-500">
            Validate it against plan-exec-v1 and build the executor prompt.
          </span>
        </span>
        <span className="shrink-0 text-sm font-bold text-primary">
          Open Bridge →
        </span>
      </button>

      {/* Warnings */}
      {result.warnings.length > 0 && (
        <section className="rounded-2xl border border-amber-700/40 bg-amber-950/20 p-4">
          <div className="mb-2 flex items-center gap-1.5 font-semibold text-amber-400">
            <AlertIcon size={14} />
            {result.warnings.length} warning
            {result.warnings.length === 1 ? "" : "s"}
          </div>
          <ul className="space-y-1 text-xs text-amber-300/80">
            {warnings.visible.map((w) => (
              <li
                key={`${w.kind}:${w.path ?? ""}:${w.message}`}
                className="break-words"
              >
                {w.path ? (
                  <span className="font-mono text-amber-200/90">
                    {w.path}:{" "}
                  </span>
                ) : null}
                {w.message}
              </li>
            ))}
          </ul>
          {warnings.isCapped && (
            <button
              type="button"
              onClick={() => setShowAllWarnings((v) => !v)}
              className="mt-2.5 rounded-md border border-amber-700/40 bg-amber-950/40 px-2.5 py-1 text-xs font-medium text-amber-300 transition-colors hover:bg-amber-900/40 hover:text-amber-200"
            >
              {showAllWarnings
                ? "Show less"
                : `Show ${fmtNum(warnings.hiddenCount)} more`}
            </button>
          )}
        </section>
      )}

      {/* Redactions */}
      {result.redactions.length > 0 && (
        <section className="rounded-2xl border border-red-700/40 bg-red-950/20 p-4">
          <div className="mb-3 flex items-center gap-1.5 font-semibold text-red-400">
            <AlertIcon size={14} />
            {result.redactions.length} secret
            {result.redactions.length === 1 ? "" : "s"} redacted
          </div>
          <div className="overflow-hidden rounded-lg border border-red-900/40 bg-red-950/30">
            <table className="w-full text-left text-xs">
              <thead className="border-b border-red-900/40 bg-red-950/40 text-[10px] uppercase tracking-wider text-red-300/70">
                <tr>
                  <th className="px-3 py-2 font-semibold">File</th>
                  <th className="px-3 py-2 font-semibold">Rule</th>
                  <th className="px-3 py-2 text-right font-semibold">Line</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-red-900/30">
                {redactions.visible.map((r, i) => (
                  <tr
                    // biome-ignore lint/suspicious/noArrayIndexKey: stable list, append-only
                    key={`${r.file}:${r.line}:${r.byteOffset}:${i}`}
                    className="text-red-200/80"
                  >
                    <td className="px-3 py-1.5 font-mono break-all">
                      {r.file}
                    </td>
                    <td className="px-3 py-1.5 font-mono">{r.ruleId}</td>
                    <td className="px-3 py-1.5 text-right font-mono">
                      {r.line}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {redactions.isCapped && (
            <button
              type="button"
              onClick={() => setShowAllRedactions((v) => !v)}
              className="mt-3 rounded-md border border-red-700/40 bg-red-950/40 px-2.5 py-1 text-xs font-medium text-red-300 transition-colors hover:bg-red-900/40 hover:text-red-200"
            >
              {showAllRedactions
                ? "Show less"
                : `Show ${fmtNum(redactions.hiddenCount)} more`}
            </button>
          )}
        </section>
      )}

      {/* Output preview */}
      <section
        aria-label={
          truncated
            ? `Output preview, truncated to the first ${fmtNum(previewLimit)} of ${fmtNum(result.output.length)} characters`
            : "Output preview"
        }
        className="space-y-2"
      >
        <div className="flex items-center justify-between">
          <SectionTitle>Output preview</SectionTitle>
          <span className="text-[11px] text-zinc-600">
            {fmtBytes(result.output.length)} · {fmtNum(result.output.length)}{" "}
            chars{truncated ? ` · showing first ${fmtNum(previewLimit)}` : ""}
          </span>
        </div>
        <pre className="max-h-96 overflow-auto rounded-xl border border-zinc-800 bg-zinc-950/70 p-4 font-mono text-[11px] leading-relaxed text-zinc-300">
          {preview}
          {truncated && (
            <span className="block pt-2 text-zinc-600">
              … {fmtBytes(result.output.length - previewLimit)} more not shown.
              Use <span className="text-zinc-400">Copy</span> or{" "}
              <span className="text-zinc-400">Save to file…</span> for the full
              output.
            </span>
          )}
        </pre>
      </section>
    </div>
  );
}
