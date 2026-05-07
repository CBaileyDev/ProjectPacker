import type { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import type { PackFormat, ProgressEvent } from "../bindings";
import { commands } from "../bindings";
import { AiContextTable } from "../components/pack/AiContextTable";
import { CopyButton } from "../components/pack/CopyButton";
import { DropOverlay } from "../components/pack/DropOverlay";
import { PhaseBreakdown } from "../components/pack/PhaseBreakdown";
import { ProgressLog } from "../components/pack/ProgressLog";
import { StatsBar } from "../components/pack/StatsBar";
import { Toggle } from "../components/pack/Toggle";
import { createPackProgressChannel } from "../lib/events";
import { useApp } from "../lib/store";
import { useDragDrop } from "../lib/use-drag-drop";

// ---------------------------------------------------------------------------
// Format display labels
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// Main screen
// ---------------------------------------------------------------------------
export default function Pack() {
  const {
    options,
    patchOptions,
    status,
    events,
    setJob,
    pushEvent,
    setResult,
    result,
    reset,
  } = useApp();

  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Single Channel for the component lifetime. Tauri's `Channel` registers
  // an internal IPC handler; reassigning `onmessage` to a no-op on `done`
  // does NOT unregister it. Reusing one channel across runs prevents the
  // O(packs run) handler leak. The channel is created lazily on first
  // `runPack` because `new Channel()` allocates a wire id even before any
  // command attaches it. Cleanup runs on unmount.
  const channelRef = useRef<Channel<ProgressEvent> | null>(null);
  const isRunning = status === "running";
  const isRunningRef = useRef(isRunning);
  isRunningRef.current = isRunning;

  useEffect(() => {
    return () => {
      // Drop the channel reference so the Tauri side can GC its handler.
      // No explicit close API; setting onmessage to a no-op + dropping the
      // reference is the best we can do.
      if (channelRef.current) {
        channelRef.current.onmessage = () => {};
        channelRef.current = null;
      }
    };
  }, []);

  const { isDragging } = useDragDrop({
    onDrop: (folderPath: string) => {
      // Ignore drops while a pack is in flight — clobbering options.target
      // mid-pack confuses the UI (in-flight pack uses server-captured target;
      // the UI would show a different one with no way to reconcile).
      if (isRunningRef.current) return;
      patchOptions({ target: { kind: "folder", value: folderPath } });
    },
  });

  async function pickFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      patchOptions({ target: { kind: "folder", value: path } });
    }
  }

  async function runPack() {
    if (isRunningRef.current) return; // double-click / pre-await reentry guard
    setErrorMsg(null);
    reset();

    // Lazily create the channel once. Subsequent runs reuse it — `onmessage`
    // is reassigned each pack to capture the fresh closure (jobId, etc.)
    // but the underlying Tauri IPC subscription stays the same.
    if (channelRef.current === null) {
      channelRef.current = createPackProgressChannel(() => {});
    }
    const channel = channelRef.current;

    // Install the real handler BEFORE awaiting packStart. Otherwise an
    // event emitted between packStart returning (server side) and the JS
    // continuation reassigning onmessage would be silently swallowed by
    // the no-op handler. The `Started` event carries `job_id`, so we can
    // capture it from inside the handler instead of waiting for
    // packStart's return value.
    let capturedJobId: string | null = null;
    channel.onmessage = (e) => {
      pushEvent(e);
      if (e.kind === "started") {
        capturedJobId = e.job_id;
      }
      if (e.kind === "done") {
        const id = capturedJobId;
        if (!id) {
          // Shouldn't happen — `started` always precedes `done`. Fall back
          // to a useful error rather than swallowing.
          setErrorMsg("internal: done without started");
          return;
        }
        (async () => {
          const r = await commands.packGetResult(id);
          if (r.status === "ok") setResult(r.data);
          else setErrorMsg(r.error.message);
        })();
      }
      if (e.kind === "error" && e.fatal) {
        setErrorMsg(e.message);
      }
    };

    const startRes = await commands.packStart(options, channel);
    if (startRes.status !== "ok") {
      setErrorMsg(startRes.error.message);
      return;
    }
    setJob(startRes.data);
  }

  const targetMode = options.target.kind;
  const targetVal = options.target.value;

  const githubUrlPattern =
    /^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+(\.git)?\/?$/;

  const isValidTarget =
    targetMode === "folder"
      ? targetVal.length > 0
      : githubUrlPattern.test(targetVal);

  function setTargetMode(mode: "folder" | "github") {
    patchOptions({ target: { kind: mode, value: "" } });
  }
  const isDone = status === "done";

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <DropOverlay visible={isDragging} />
      <div className="mx-auto max-w-2xl space-y-6 p-6">
        {/* ── Header ── */}
        <div>
          <h1 className="text-2xl font-bold tracking-tight">ProjectPacker</h1>
          <p className="mt-1 text-sm text-zinc-400">
            Pack a codebase into a single file for AI consumption.
          </p>
        </div>

        {/* ── Target ── */}
        <section className="space-y-2">
          <h3 className="block text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Target
          </h3>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setTargetMode("folder")}
              className={`rounded px-3 py-1 text-sm transition-colors ${
                targetMode === "folder"
                  ? "bg-emerald-700 text-white"
                  : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
              }`}
            >
              Folder
            </button>
            <button
              type="button"
              onClick={() => setTargetMode("github")}
              className={`rounded px-3 py-1 text-sm transition-colors ${
                targetMode === "github"
                  ? "bg-emerald-700 text-white"
                  : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
              }`}
            >
              GitHub URL
            </button>
          </div>

          {targetMode === "folder" ? (
            <div className="flex gap-2">
              <input
                className="flex-1 rounded border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm focus:border-zinc-500 focus:outline-none"
                value={targetVal}
                placeholder="/path/to/project"
                onChange={(e) =>
                  patchOptions({
                    target: { kind: "folder", value: e.target.value },
                  })
                }
              />
              <button
                type="button"
                className="rounded border border-zinc-600 bg-zinc-700 px-3 py-2 text-sm hover:bg-zinc-600"
                onClick={pickFolder}
              >
                Browse…
              </button>
            </div>
          ) : (
            <div>
              <input
                className={`w-full rounded border bg-zinc-800 px-3 py-2 text-sm focus:outline-none ${
                  targetVal && !isValidTarget
                    ? "border-red-600 focus:border-red-500"
                    : "border-zinc-700 focus:border-zinc-500"
                }`}
                value={targetVal}
                placeholder="https://github.com/owner/repo"
                onChange={(e) =>
                  patchOptions({
                    target: { kind: "github", value: e.target.value },
                  })
                }
              />
              {targetVal && !isValidTarget && (
                <div className="mt-1 text-xs text-red-400">
                  Enter a valid GitHub repo URL (https://github.com/owner/repo)
                </div>
              )}
            </div>
          )}
        </section>

        {/* ── Goal ── */}
        <section className="space-y-2">
          <h3 className="block text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Goal / Task Description
          </h3>
          <textarea
            className="h-20 w-full resize-none rounded border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm focus:border-zinc-500 focus:outline-none"
            value={options.goal}
            placeholder="Describe what you want to build or fix…"
            onChange={(e) => patchOptions({ goal: e.target.value })}
          />
        </section>

        {/* ── Options ── */}
        <section className="space-y-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Options
          </div>

          <div className="grid grid-cols-2 gap-x-8 gap-y-3">
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
              hint="counts via 7 model tokenizers (see AI table)"
              checked={options.countTokens}
              onChange={(v) => patchOptions({ countTokens: v })}
            />
          </div>

          <div className="flex flex-wrap items-center gap-6">
            <label className="flex items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                Output Format
              </span>
              <select
                className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-sm focus:border-zinc-500 focus:outline-none"
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

            <label className="flex items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                Max File Size
              </span>
              <input
                type="number"
                min={1}
                max={102_400}
                className="w-20 rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-sm focus:border-zinc-500 focus:outline-none"
                value={options.maxFileSizeKb}
                onChange={(e) => {
                  // `Number("")` is NaN; coerce to a sane minimum so we don't
                  // serialize NaN into the persisted store (it round-trips to
                  // null and breaks the backend on the next pack).
                  const parsed = Number(e.target.value);
                  patchOptions({
                    maxFileSizeKb: Number.isFinite(parsed) && parsed > 0 ? parsed : 1,
                  });
                }}
              />
              <span className="text-xs text-zinc-500">KB</span>
            </label>
          </div>
        </section>

        {/* ── Pack button ── */}
        <button
          type="button"
          className="w-full rounded bg-emerald-700 py-3 text-sm font-semibold transition-colors hover:bg-emerald-600 disabled:cursor-not-allowed disabled:opacity-40"
          onClick={runPack}
          disabled={isRunning || !isValidTarget}
        >
          {isRunning ? "Packing…" : "Pack"}
        </button>

        {/* ── Error ── */}
        {errorMsg && (
          <div className="rounded border border-red-600 bg-red-950/40 px-4 py-3 text-sm text-red-300">
            {errorMsg}
          </div>
        )}

        {/* ── Progress ── */}
        {isRunning && <ProgressLog events={events} />}

        {/* ── Results ── */}
        {isDone && result && (
          <div className="space-y-4">
            <div className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              Result
            </div>

            <StatsBar stats={result.stats} />
            <PhaseBreakdown stats={result.stats} />

            <div className="flex flex-wrap gap-2">
              <CopyButton
                label={COPY_BUTTON_LABELS[options.format]}
                text={result.output}
              />
              <CopyButton
                label="Copy Claude Code Prompt"
                text={result.claudeCodePrompt}
              />
              <button
                type="button"
                className="rounded border border-zinc-600 bg-zinc-800 px-4 py-2 text-sm hover:bg-zinc-700"
                onClick={() => reset()}
              >
                New Pack
              </button>
            </div>

            {result.warnings.length > 0 && (
              <div className="rounded border border-amber-700 bg-amber-950/30 px-4 py-3 text-sm">
                <div className="mb-1 font-semibold text-amber-400">
                  Warnings
                </div>
                {result.warnings.map((w) => (
                  <div
                    key={`${w.kind}:${w.path ?? ""}:${w.message}`}
                    className="text-xs text-amber-300"
                  >
                    {w.message}
                  </div>
                ))}
              </div>
            )}

            <AiContextTable tokensPerModel={result.stats.tokensPerModel} />
          </div>
        )}
      </div>
    </div>
  );
}
