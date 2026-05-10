import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import type React from "react";
import { useEffect, useMemo, useState } from "react";
import type { PackFormat, PackOptions, PackResult } from "../bindings";
import { AiContextTable } from "../components/pack/AiContextTable";
import { CopyButton } from "../components/pack/CopyButton";
import { DropOverlay } from "../components/pack/DropOverlay";
import { GithubConnector } from "../components/pack/GithubConnector";
import {
  AlertIcon,
  FileTextIcon,
  FolderIcon,
  GithubIcon,
  LoaderIcon,
  PackageIcon,
  PlayIcon,
  SettingsIcon,
  SparklesIcon,
  XIcon,
} from "../components/pack/icons";
import { PhaseBreakdown } from "../components/pack/PhaseBreakdown";
import { ProgressLog } from "../components/pack/ProgressLog";
import { SaveButton } from "../components/pack/SaveButton";
import { Settings } from "../components/pack/Settings";
import { StatsBar } from "../components/pack/StatsBar";
import { Toggle } from "../components/pack/Toggle";
import { fmtBytes, fmtNum } from "../lib/format";
import {
  fadeUp,
  prefersReducedMotion,
  springButton,
  staggerContainer,
} from "../lib/motion";
import { useApp } from "../lib/store";
import { useDragDrop } from "../lib/use-drag-drop";
import { usePackJob } from "../lib/use-pack-job";

const FORMAT_LABELS: Record<PackFormat, string> = {
  xml: "XML  (Claude Code / Grok)",
  markdown: "Markdown",
  plainText: "Plain Text",
};

const COPY_BUTTON_LABELS: Record<PackFormat, string> = {
  xml: "Copy Pack XML",
  markdown: "Copy Pack Markdown",
  plainText: "Copy Plain Text",
};

const SAVE_FILENAMES: Record<PackFormat, string> = {
  xml: "pack.xml",
  markdown: "pack.md",
  plainText: "pack.txt",
};

const GITHUB_URL_PATTERN =
  /^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+(\.git)?\/?$/;

const MAX_FILE_SIZE_KB = 102_400;

type PackTab = "packer" | "results" | "github" | "settings";
const TAB_LABELS: Record<PackTab, string> = {
  packer: "Packer",
  results: "Results",
  github: "GitHub",
  settings: "Settings",
};
const TAB_HEADLINES: Record<PackTab, string> = {
  packer: "Pack a project",
  results: "Pack results",
  github: "Browse your GitHub repositories",
  settings: "Settings",
};

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="block text-xs font-semibold uppercase tracking-wider text-zinc-500">
      {children}
    </h3>
  );
}

export default function Pack() {
  const { options, patchOptions, status, events, result, reset } = useApp();
  const { run: runPack, errorMsg, dismissError, isRunning } = usePackJob();

  const { isDragging, dropState } = useDragDrop({
    onDrop: (folderPath: string) => {
      if (isRunning) return;
      patchOptions({ target: { kind: "folder", value: folderPath } });
    },
  });

  async function pickFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      patchOptions({ target: { kind: "folder", value: path } });
    }
  }

  const targetMode = options.target.kind;
  const targetVal = options.target.value;

  const isValidTarget = useMemo(
    () =>
      targetMode === "folder"
        ? targetVal.length > 0
        : GITHUB_URL_PATTERN.test(targetVal),
    [targetMode, targetVal],
  );

  function setTargetMode(mode: "folder" | "github") {
    patchOptions({ target: { kind: mode, value: "" } });
  }

  const isDone = status === "done";
  const showResultSkeleton = isRunning && !result;

  const [activeTab, setActiveTab] = useState<PackTab>("packer");

  const tabs: Array<{
    id: PackTab;
    label: string;
    description: string;
    Icon: React.FC<{ size?: number; className?: string }>;
    disabled?: boolean;
  }> = [
    {
      id: "packer",
      label: "Packer",
      description: isRunning ? "Packing in progress" : "Configure and run",
      Icon: PackageIcon,
    },
    {
      id: "results",
      label: "Results",
      description: result
        ? `${fmtNum(result.stats.filesIncluded)} files packed`
        : "Detailed view after a run",
      Icon: FileTextIcon,
      disabled: !result,
    },
    {
      id: "github",
      label: "GitHub",
      description: "Pick a repo to pack",
      Icon: GithubIcon,
    },
    {
      id: "settings",
      label: "Settings",
      description: "Tokens and preferences",
      Icon: SettingsIcon,
    },
  ];

  // Auto-route to Results when output lands.
  useEffect(() => {
    if (isDone && result) {
      setActiveTab("results");
    }
  }, [isDone, result]);

  function handlePack() {
    runPack();
  }

  function selectGithubRepo(htmlUrl: string) {
    patchOptions({ target: { kind: "github", value: htmlUrl } });
    setActiveTab("packer");
  }

  return (
    <div className="min-h-screen text-zinc-100">
      <DropOverlay visible={isDragging} dropState={dropState} />

      <motion.div
        className="mx-auto grid min-h-screen max-w-6xl gap-6 p-6 lg:grid-cols-[280px_minmax(0,1fr)]"
        variants={staggerContainer}
        initial="hidden"
        animate="visible"
      >
        {/* ── Sidebar ── */}
        <motion.aside
          variants={fadeUp}
          className="flex flex-col rounded-2xl border border-zinc-800/80 bg-zinc-950/70 p-4 shadow-2xl shadow-black/20 backdrop-blur"
        >
          <div className="mb-6 flex items-start gap-3 px-1">
            <div className="mt-0.5 flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 ring-1 ring-emerald-500/20">
              <PackageIcon size={21} className="text-emerald-400" />
            </div>
            <div>
              <h1 className="text-xl font-bold tracking-tight">ProjectPacker</h1>
              <p className="mt-1 text-xs leading-relaxed text-zinc-500">
                Pack a codebase into a single AI-ready file.
              </p>
            </div>
          </div>

          <nav className="space-y-2" aria-label="App sections">
            {tabs.map(({ id, label, description, Icon, disabled }) => {
              const selected = activeTab === id;
              return (
                <button
                  key={id}
                  type="button"
                  disabled={disabled}
                  onClick={() => setActiveTab(id)}
                  className={`relative flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition-all duration-200 ${
                    selected
                      ? "bg-emerald-500/15 text-emerald-100 ring-1 ring-emerald-500/30"
                      : disabled
                        ? "cursor-not-allowed text-zinc-600"
                        : "text-zinc-400 hover:bg-zinc-800/70 hover:text-zinc-100"
                  }`}
                  aria-current={selected ? "page" : undefined}
                >
                  {selected && (
                    <motion.span
                      layoutId="active-pack-tab"
                      className="absolute left-0 top-2 h-[calc(100%-1rem)] w-1 rounded-r-full bg-emerald-400"
                      transition={{ type: "spring", stiffness: 420, damping: 34 }}
                    />
                  )}
                  <span
                    className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${
                      selected ? "bg-emerald-500/20" : "bg-zinc-900/80"
                    }`}
                  >
                    <Icon size={16} />
                  </span>
                  <span className="min-w-0">
                    <span className="block text-sm font-semibold">{label}</span>
                    <span className="mt-0.5 block truncate text-xs text-zinc-500">
                      {description}
                    </span>
                  </span>
                </button>
              );
            })}
          </nav>

          {isRunning && (
            <div className="mt-auto pt-6">
              <div className="flex items-center gap-2 rounded-lg border border-emerald-700/40 bg-emerald-950/20 px-3 py-2.5 text-xs text-emerald-300">
                <motion.span
                  aria-hidden="true"
                  animate={{ rotate: 360 }}
                  transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
                >
                  <LoaderIcon size={12} />
                </motion.span>
                Packing in progress
              </div>
            </div>
          )}
        </motion.aside>

        {/* ── Main ── */}
        <motion.main
          variants={fadeUp}
          className="min-w-0 rounded-2xl border border-zinc-800/80 bg-zinc-950/55 p-6 shadow-2xl shadow-black/20 backdrop-blur"
        >
          <div className="mb-5">
            <p className="text-xs font-semibold uppercase tracking-wider text-emerald-400">
              {TAB_LABELS[activeTab]}
            </p>
            <h2 className="mt-1 text-2xl font-bold tracking-tight">
              {TAB_HEADLINES[activeTab]}
            </h2>
          </div>

          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={prefersReducedMotion ? false : { opacity: 0, x: 18 }}
              animate={{ opacity: 1, x: 0 }}
              exit={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, x: -18 }}
              transition={{
                duration: prefersReducedMotion ? 0 : 0.22,
                ease: [0.22, 1, 0.36, 1],
              }}
              className="min-h-[520px]"
            >
              {/* ── Packer tab — original single-column UX ── */}
              {activeTab === "packer" && (
                <div className="space-y-6">
                  {/* Target */}
                  <section className="space-y-3">
                    <SectionTitle>Target</SectionTitle>

                    <div className="flex w-fit gap-1.5 rounded-lg border border-zinc-700/50 bg-zinc-800/60 p-1">
                      <button
                        type="button"
                        onClick={() => setTargetMode("folder")}
                        aria-pressed={targetMode === "folder"}
                        className={`flex items-center gap-1.5 rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200 ${
                          targetMode === "folder"
                            ? "bg-emerald-600 text-white shadow-lg shadow-emerald-900/30"
                            : "text-zinc-400 hover:text-zinc-200"
                        }`}
                      >
                        <FolderIcon size={14} />
                        Folder
                      </button>
                      <button
                        type="button"
                        onClick={() => setTargetMode("github")}
                        aria-pressed={targetMode === "github"}
                        className={`flex items-center gap-1.5 rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200 ${
                          targetMode === "github"
                            ? "bg-emerald-600 text-white shadow-lg shadow-emerald-900/30"
                            : "text-zinc-400 hover:text-zinc-200"
                        }`}
                      >
                        <GithubIcon size={14} />
                        GitHub URL
                      </button>
                    </div>

                    <AnimatePresence mode="wait">
                      {targetMode === "folder" ? (
                        <motion.div
                          key="folder"
                          className="flex gap-2"
                          initial={prefersReducedMotion ? false : { opacity: 0, y: -6 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
                          transition={{ duration: prefersReducedMotion ? 0 : 0.2 }}
                        >
                          <input
                            className="flex-1 rounded-lg border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:border-emerald-500/50 focus:outline-none"
                            value={targetVal}
                            placeholder="/path/to/project"
                            aria-label="Folder path"
                            onChange={(e) =>
                              patchOptions({
                                target: { kind: "folder", value: e.target.value },
                              })
                            }
                          />
                          <motion.button
                            type="button"
                            className="rounded-lg border border-zinc-600 bg-zinc-700 px-4 py-2.5 text-sm text-zinc-200 hover:bg-zinc-600 transition-colors"
                            onClick={pickFolder}
                            whileTap={springButton}
                          >
                            Browse…
                          </motion.button>
                        </motion.div>
                      ) : (
                        <motion.div
                          key="github"
                          initial={prefersReducedMotion ? false : { opacity: 0, y: -6 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
                          transition={{ duration: prefersReducedMotion ? 0 : 0.2 }}
                        >
                          <div className="flex gap-2">
                            <input
                              className={`flex-1 rounded-lg border bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:outline-none ${
                                targetVal && !isValidTarget
                                  ? "border-red-600 focus:border-red-500"
                                  : "border-zinc-700 focus:border-emerald-500/50"
                              }`}
                              value={targetVal}
                              placeholder="https://github.com/owner/repo"
                              aria-label="GitHub repository URL"
                              aria-invalid={Boolean(targetVal) && !isValidTarget}
                              onChange={(e) =>
                                patchOptions({
                                  target: { kind: "github", value: e.target.value },
                                })
                              }
                            />
                            <motion.button
                              type="button"
                              className="rounded-lg border border-zinc-600 bg-zinc-700 px-4 py-2.5 text-sm text-zinc-200 hover:bg-zinc-600 transition-colors"
                              onClick={() => setActiveTab("github")}
                              whileTap={springButton}
                              title="Pick a repo from the GitHub tab"
                            >
                              Browse…
                            </motion.button>
                          </div>
                          {targetVal && !isValidTarget && (
                            <motion.div
                              className="mt-1.5 text-xs text-red-400"
                              role="alert"
                              initial={{ opacity: 0, y: -4 }}
                              animate={{ opacity: 1, y: 0 }}
                            >
                              Enter a valid GitHub repo URL, such as
                              https://github.com/owner/repo
                            </motion.div>
                          )}
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </section>

                  {/* Goal */}
                  <section className="space-y-3">
                    <SectionTitle>Goal / Task Description</SectionTitle>
                    <textarea
                      className="h-20 w-full resize-none rounded-lg border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:border-emerald-500/50 focus:outline-none"
                      value={options.goal}
                      placeholder="Describe what you want to build or fix…"
                      aria-label="Goal or task description"
                      onChange={(e) => patchOptions({ goal: e.target.value })}
                    />
                  </section>

                  {/* Options */}
                  <section className="space-y-4 rounded-xl border border-zinc-700/50 bg-zinc-800/30 p-5">
                    <div className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                      Options
                    </div>

                    <div className="grid gap-3 md:grid-cols-2">
                      <Toggle
                        label="Compress to skeleton"
                        hint="strips bodies, keeps signatures"
                        checked={options.compress}
                        onChange={(v) => patchOptions({ compress: v })}
                      />
                      <Toggle
                        label="Remove comments"
                        hint="tree-sitter: Rust/Py/JS/TS"
                        checked={options.removeComments}
                        onChange={(v) => patchOptions({ removeComments: v })}
                      />
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

                    <div className="grid gap-4 rounded-xl border border-zinc-800 bg-zinc-950/50 p-4 md:grid-cols-2">
                      <label className="flex flex-wrap items-center gap-2">
                        <span className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                          Output Format
                        </span>
                        <select
                          className="rounded-lg border border-zinc-700 bg-zinc-800/60 px-2.5 py-1.5 text-sm text-zinc-100 focus:border-emerald-500/50 focus:outline-none"
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
                          className="w-20 rounded-lg border border-zinc-700 bg-zinc-800/60 px-2.5 py-1.5 text-sm text-zinc-100 focus:border-emerald-500/50 focus:outline-none"
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
                  </section>

                  {/* Pack button */}
                  <motion.button
                    type="button"
                    className={`flex w-full items-center justify-center gap-2 rounded-xl py-3.5 text-sm font-semibold transition-all duration-200 ${
                      isRunning || !isValidTarget
                        ? "cursor-not-allowed bg-emerald-800/50 text-emerald-300/50"
                        : "bg-emerald-600 text-white shadow-lg shadow-emerald-900/30 hover:bg-emerald-500 hover:shadow-emerald-900/40"
                    }`}
                    onClick={handlePack}
                    disabled={isRunning || !isValidTarget}
                    aria-busy={isRunning}
                    whileTap={!isRunning && isValidTarget ? { scale: 0.98 } : undefined}
                  >
                    <AnimatePresence mode="wait" initial={false}>
                      {isRunning ? (
                        <motion.span
                          key="running"
                          className="flex items-center gap-2"
                          initial={{ opacity: 0, y: 8 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={{ opacity: 0, y: -8 }}
                          transition={{ duration: 0.2 }}
                        >
                          <motion.span
                            aria-hidden="true"
                            animate={{ rotate: 360 }}
                            transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
                          >
                            <LoaderIcon size={16} />
                          </motion.span>
                          Packing…
                        </motion.span>
                      ) : (
                        <motion.span
                          key="idle"
                          className="flex items-center gap-2"
                          initial={{ opacity: 0, y: 8 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={{ opacity: 0, y: -8 }}
                          transition={{ duration: 0.2 }}
                        >
                          <PlayIcon size={16} />
                          Pack
                        </motion.span>
                      )}
                    </AnimatePresence>
                  </motion.button>

                  {/* Error */}
                  <AnimatePresence>
                    {errorMsg && (
                      <motion.div
                        role="alert"
                        className="flex items-start gap-3 rounded-xl border border-red-600/40 bg-red-950/40 px-4 py-3 text-sm text-red-300"
                        initial={{ opacity: 0, y: -8, scale: 0.98 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: -8, scale: 0.98 }}
                        transition={{ type: "spring", stiffness: 400, damping: 28 }}
                      >
                        <AlertIcon size={16} className="mt-0.5 shrink-0 text-red-400" />
                        <div className="flex-1 break-words">{errorMsg}</div>
                        <button
                          type="button"
                          onClick={dismissError}
                          aria-label="Dismiss error"
                          className="-mr-1 -mt-1 shrink-0 rounded p-1 text-red-300/80 hover:bg-red-900/40 hover:text-red-200 transition-colors"
                        >
                          <XIcon size={14} />
                        </button>
                      </motion.div>
                    )}
                  </AnimatePresence>

                  {/* Progress while running */}
                  <AnimatePresence>
                    {isRunning && (
                      <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: "auto" }}
                        exit={{ opacity: 0, height: 0 }}
                        transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
                      >
                        <ProgressLog events={events} />
                      </motion.div>
                    )}
                  </AnimatePresence>

                  {/* Result skeleton */}
                  <AnimatePresence>
                    {showResultSkeleton && (
                      <motion.div
                        className="space-y-5"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.3, delay: 0.05 }}
                      >
                        <StatsBar stats={null} loading />
                      </motion.div>
                    )}
                  </AnimatePresence>

                  {/* Result summary inline (full deep-dive lives in Results tab) */}
                  <AnimatePresence>
                    {isDone && result && (
                      <motion.div
                        className="space-y-4"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        transition={{ duration: 0.4, delay: 0.1 }}
                      >
                        <div className="flex items-center gap-2">
                          <SparklesIcon size={14} className="text-emerald-400" />
                          <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
                            Done
                          </span>
                        </div>
                        <StatsBar stats={result.stats} />
                        <div className="flex flex-wrap gap-2.5">
                          <CopyButton
                            label={COPY_BUTTON_LABELS[options.format]}
                            text={result.output}
                          />
                          <SaveButton
                            label="Save to file…"
                            suggestedFilename={SAVE_FILENAMES[options.format]}
                            text={result.output}
                          />
                          <motion.button
                            type="button"
                            className="flex items-center gap-1.5 rounded-lg border border-emerald-600/60 bg-emerald-500/10 px-4 py-2.5 text-sm text-emerald-300 hover:bg-emerald-500/20 transition-colors"
                            onClick={() => setActiveTab("results")}
                            whileTap={springButton}
                          >
                            <FileTextIcon size={14} />
                            Open Results tab
                          </motion.button>
                          <motion.button
                            type="button"
                            className="flex items-center gap-1.5 rounded-lg border border-zinc-600/80 bg-zinc-800 px-4 py-2.5 text-sm text-zinc-300 hover:bg-zinc-700 hover:text-white transition-colors"
                            onClick={() => reset()}
                            whileTap={springButton}
                          >
                            New Pack
                          </motion.button>
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              )}

              {/* ── Results tab — deep dive ── */}
              {activeTab === "results" && (
                <ResultsTab
                  result={result}
                  options={options}
                  reset={reset}
                  switchToPacker={() => setActiveTab("packer")}
                />
              )}

              {/* ── GitHub tab ── */}
              {activeTab === "github" && (
                <GithubConnector
                  onSelectRepo={selectGithubRepo}
                  onGoToSettings={() => setActiveTab("settings")}
                />
              )}

              {/* ── Settings tab ── */}
              {activeTab === "settings" && <Settings />}
            </motion.div>
          </AnimatePresence>
        </motion.main>
      </motion.div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────
// Results tab — extracted for clarity. Shows the full StatsBar, phase
// breakdown, redaction list, AI compatibility table, output preview, and
// the full button row. The Packer tab keeps a slimmed summary instead.
// ─────────────────────────────────────────────────────────────────────────

interface ResultsTabProps {
  result: PackResult | null;
  options: PackOptions;
  reset: () => void;
  switchToPacker: () => void;
}

function ResultsTab({ result, options, reset, switchToPacker }: ResultsTabProps) {
  if (!result) {
    return (
      <div className="flex min-h-[360px] flex-col items-center justify-center rounded-2xl border border-dashed border-zinc-700/70 bg-zinc-900/25 px-6 text-center">
        <FileTextIcon size={28} className="text-zinc-500" />
        <h3 className="mt-4 text-base font-semibold text-zinc-200">
          No pack results yet
        </h3>
        <p className="mt-2 max-w-sm text-sm leading-relaxed text-zinc-500">
          Run a pack from the Packer tab. When it completes, this view will
          show stats, phase timing, AI compatibility, redactions, and a
          preview of the output.
        </p>
        <motion.button
          type="button"
          className="mt-5 flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2.5 text-sm font-semibold text-white"
          onClick={switchToPacker}
          whileTap={springButton}
        >
          <PackageIcon size={14} />
          Open Packer
        </motion.button>
      </div>
    );
  }

  const previewLimit = 8000;
  const preview = result.output.slice(0, previewLimit);
  const truncated = result.output.length > previewLimit;

  return (
    <div className="space-y-6">
      {/* Stats + phase breakdown */}
      <section className="space-y-4">
        <StatsBar stats={result.stats} />
        <PhaseBreakdown stats={result.stats} />
      </section>

      {/* Action buttons */}
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
          label="Copy Claude Code Prompt"
          text={result.claudeCodePrompt}
        />
        <motion.button
          type="button"
          className="flex items-center gap-1.5 rounded-lg border border-zinc-600/80 bg-zinc-800 px-4 py-2.5 text-sm text-zinc-300 hover:bg-zinc-700 hover:text-white transition-colors"
          onClick={() => reset()}
          whileTap={springButton}
        >
          New Pack
        </motion.button>
      </section>

      {/* AI compatibility */}
      <AiContextTable tokensPerModel={result.stats.tokensPerModel} />

      {/* Warnings */}
      {result.warnings.length > 0 && (
        <section className="rounded-2xl border border-amber-700/40 bg-amber-950/20 p-4">
          <div className="mb-2 flex items-center gap-1.5 font-semibold text-amber-400">
            <AlertIcon size={14} />
            {result.warnings.length} warning
            {result.warnings.length === 1 ? "" : "s"}
          </div>
          <ul className="space-y-1 text-xs text-amber-300/80">
            {result.warnings.map((w) => (
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
                {result.redactions.map((r, i) => (
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
        </section>
      )}

      {/* Output preview */}
      <section className="space-y-2">
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
