import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { useMemo } from "react";
import type { PackFormat } from "../bindings";
import { GoalSection } from "../components/home/GoalSection";
import { OnboardingCard } from "../components/home/OnboardingCard";
import { TargetSection } from "../components/home/TargetSection";
import { CompressionPanel } from "../components/pack/CompressionPanel";
import {
  AlertIcon,
  ChevronDownIcon,
  FolderIcon,
  GithubIcon,
  LoaderIcon,
  PlayIcon,
  XIcon,
} from "../components/pack/icons";
import { ProgressLog } from "../components/pack/ProgressLog";
import { Toggle } from "../components/pack/Toggle";
import { fadeUp, prefersReducedMotion } from "../lib/motion";
import { usePackJobContext } from "../lib/pack-job-context";
import {
  FORMAT_LABELS,
  isValidTargetValue,
  MAX_FILE_SIZE_KB,
} from "../lib/pack-meta";
import { applyPreset, matchPreset, PRESETS } from "../lib/presets";
import {
  useApp,
  usePackEvents,
  usePackOptions,
  usePackProgress,
} from "../lib/store";
import {
  isMacPlatform,
  useKeyboardShortcuts,
} from "../lib/use-keyboard-shortcuts";

/** Derive a short display label for a recent target. */
function recentLabel(value: string, kind: "folder" | "github"): string {
  if (kind === "github") {
    return value
      .replace(/^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)/, "")
      .replace(/(\.git)?\/?$/, "");
  }
  const parts = value.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? value;
}

export default function Home() {
  const { options, patchOptions } = usePackOptions();
  const { status, result } = usePackProgress();
  const setSheet = useApp((s) => s.setSheet);
  const advancedOpen = useApp((s) => s.advancedOpen);
  const setAdvancedOpen = useApp((s) => s.setAdvancedOpen);
  const recentTargets = useApp((s) => s.recentTargets);
  const events = usePackEvents();
  const {
    run: runPack,
    isRunning,
    errorMsg,
    dismissError,
  } = usePackJobContext();

  const isValidTarget = useMemo(
    () => isValidTargetValue(options.target.kind, options.target.value),
    [options.target.kind, options.target.value],
  );
  const activePreset = useMemo(() => matchPreset(options), [options]);
  const isCancelled = status === "cancelled";
  const showOnboarding =
    status === "idle" && !result && options.target.value.length === 0;

  useKeyboardShortcuts({
    "mod+enter": () => {
      if (!isRunning && isValidTarget) runPack();
    },
  });

  return (
    <m.div
      className="mx-auto w-full max-w-2xl space-y-7 px-6 pb-16 pt-10"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="text-center">
        <h1 className="text-3xl font-bold tracking-tight">
          What are we packing?
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          Turn any repo into a planner-ready context pack.
        </p>
      </div>

      <TargetSection onBrowseGithub={() => setSheet("github")} />
      <GoalSection />

      {/* Preset chips */}
      <section className="space-y-3">
        <div className="flex flex-wrap items-center justify-center gap-2">
          {PRESETS.map((p) => {
            const on = activePreset === p.id;
            return (
              <button
                key={p.id}
                type="button"
                title={p.description}
                aria-pressed={on}
                onClick={() => patchOptions(applyPreset(options, p))}
                className={`rounded-full border px-4 py-1.5 text-xs font-medium transition-colors ${
                  on
                    ? "border-primary/50 bg-primary/10 text-primary"
                    : "border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"
                }`}
              >
                {p.label}
              </button>
            );
          })}
          <button
            type="button"
            aria-pressed={advancedOpen}
            onClick={() => setAdvancedOpen(!advancedOpen)}
            className={`flex items-center gap-1 rounded-full border px-4 py-1.5 text-xs font-medium transition-colors ${
              advancedOpen || activePreset === null
                ? "border-primary/50 bg-primary/10 text-primary"
                : "border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"
            }`}
          >
            Custom
            <ChevronDownIcon
              size={12}
              className={`transition-transform ${advancedOpen ? "rotate-180" : ""}`}
            />
          </button>
        </div>
      </section>

      {/* Advanced options */}
      <AnimatePresence initial={false}>
        {advancedOpen && (
          <m.section
            key="advanced"
            className="overflow-hidden"
            initial={prefersReducedMotion ? false : { height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={
              prefersReducedMotion ? { opacity: 0 } : { height: 0, opacity: 0 }
            }
            transition={{ duration: prefersReducedMotion ? 0 : 0.25 }}
          >
            <div className="space-y-4 rounded-xl border border-white/10 bg-white/[0.03] p-5">
              <div className="grid gap-3 md:grid-cols-2">
                <Toggle
                  label="Respect .gitignore"
                  checked={options.respectGitignore}
                  onChange={(v) => patchOptions({ respectGitignore: v })}
                />
                <Toggle
                  label="Scan for secrets"
                  checked={options.secretScan}
                  onChange={(v) => patchOptions({ secretScan: v })}
                />
                <Toggle
                  label="Count tokens"
                  hint="7 model tokenizers"
                  checked={options.countTokens}
                  onChange={(v) => patchOptions({ countTokens: v })}
                />
              </div>

              <CompressionPanel />

              <div className="grid gap-4 rounded-xl border border-white/10 bg-black/20 p-4 md:grid-cols-2">
                <label className="flex flex-wrap items-center gap-2">
                  <span className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                    Output Format
                  </span>
                  <select
                    className="rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 text-sm text-zinc-100 focus:border-primary/50 focus:outline-none"
                    value={options.format}
                    onChange={(e) =>
                      patchOptions({ format: e.target.value as PackFormat })
                    }
                  >
                    {(Object.keys(FORMAT_LABELS) as PackFormat[]).map((f) => (
                      <option key={f} value={f}>
                        {FORMAT_LABELS[f]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-wrap items-center gap-2">
                  <span className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                    Max File Size
                  </span>
                  <input
                    type="number"
                    min={1}
                    max={MAX_FILE_SIZE_KB}
                    className="w-20 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 text-sm text-zinc-100 focus:border-primary/50 focus:outline-none"
                    value={options.maxFileSizeKb}
                    onChange={(e) => {
                      const parsed = Number(e.target.value);
                      const clamped =
                        Number.isFinite(parsed) && parsed > 0
                          ? Math.min(parsed, MAX_FILE_SIZE_KB)
                          : 1;
                      patchOptions({ maxFileSizeKb: clamped });
                    }}
                  />
                  <span className="text-xs text-zinc-500">KB</span>
                </label>
              </div>
            </div>
          </m.section>
        )}
      </AnimatePresence>

      {/* Pack button */}
      <m.button
        type="button"
        className={`flex w-full items-center justify-center gap-2 rounded-xl py-3.5 text-sm font-semibold transition-all duration-200 ${
          isRunning || !isValidTarget
            ? "cursor-not-allowed bg-primary/20 text-primary/50"
            : "bg-primary text-primary-foreground shadow-lg shadow-black/30 hover:bg-primary/90"
        }`}
        onClick={() => runPack()}
        disabled={isRunning || !isValidTarget}
        aria-busy={isRunning}
        whileTap={!isRunning && isValidTarget ? { scale: 0.98 } : undefined}
      >
        {isRunning ? (
          <>
            <m.span
              aria-hidden="true"
              animate={{ rotate: 360 }}
              transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
            >
              <LoaderIcon size={16} />
            </m.span>
            Packing…
          </>
        ) : (
          <>
            <PlayIcon size={16} />
            Pack
            <kbd className="ml-0.5 rounded border border-black/20 bg-black/10 px-1.5 py-0.5 font-sans text-[10px] font-medium leading-none opacity-70">
              {isMacPlatform() ? "⌘↵" : "Ctrl↵"}
            </kbd>
          </>
        )}
      </m.button>

      {/* Error banner */}
      <AnimatePresence>
        {errorMsg && (
          <m.div
            role="alert"
            className="flex items-start gap-3 rounded-xl border border-red-600/40 bg-red-950/40 px-4 py-3 text-sm text-red-300"
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
          >
            <AlertIcon size={16} className="mt-0.5 shrink-0 text-red-400" />
            <div className="flex-1 break-words">{errorMsg}</div>
            <button
              type="button"
              onClick={dismissError}
              aria-label="Dismiss error"
              className="-mr-1 -mt-1 shrink-0 rounded p-1 text-red-300/80 transition-colors hover:bg-red-900/40 hover:text-red-200"
            >
              <XIcon size={14} />
            </button>
          </m.div>
        )}
      </AnimatePresence>

      {/* Cancelled notice */}
      <AnimatePresence>
        {isCancelled && (
          <m.div
            role="status"
            className="flex items-center gap-3 rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3 text-sm text-zinc-400"
            initial={prefersReducedMotion ? false : { opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
          >
            <XIcon size={16} className="shrink-0 text-zinc-500" />
            Pack cancelled. Adjust your options and pack again whenever you're
            ready.
          </m.div>
        )}
      </AnimatePresence>

      {/* Last-run log — after an error/cancel the ProgressLog stays
          reachable from Home until the next pack starts. No churn concern:
          events only update while running, and running renders Packing. */}
      {(status === "error" || status === "cancelled") && events.length > 0 && (
        <details className="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3">
          <summary className="cursor-pointer select-none text-xs font-semibold text-zinc-400">
            Last run log
          </summary>
          <div className="pt-3">
            <ProgressLog events={events} />
          </div>
        </details>
      )}

      {/* Recents */}
      {recentTargets.length > 0 && (
        <section className="space-y-2.5">
          <div className="text-center text-[10px] font-semibold uppercase tracking-[0.18em] text-zinc-600">
            Recent
          </div>
          <div className="flex flex-wrap justify-center gap-2">
            {recentTargets.map((r) => (
              <button
                key={`${r.kind}:${r.value}`}
                type="button"
                title={r.value}
                onClick={() => patchOptions({ target: { ...r } })}
                className="flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-xs text-zinc-300 transition-colors hover:border-primary/40 hover:text-zinc-100"
              >
                {r.kind === "github" ? (
                  <GithubIcon size={12} className="text-primary" />
                ) : (
                  <FolderIcon size={12} className="text-primary" />
                )}
                {recentLabel(r.value, r.kind)}
              </button>
            ))}
          </div>
        </section>
      )}

      <AnimatePresence>{showOnboarding && <OnboardingCard />}</AnimatePresence>
    </m.div>
  );
}
