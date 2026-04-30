import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { commands } from "../bindings";
import type { PackFormat, PackStats, ProgressEvent } from "../bindings";
import { useApp } from "../lib/store";
import { subscribePackProgress } from "../lib/events";

// ---------------------------------------------------------------------------
// AI context-window compatibility data (as of mid-2025)
// ---------------------------------------------------------------------------
const AI_MODELS: { name: string; context: number }[] = [
  { name: "GPT-4o / GPT-4o mini", context: 128_000 },
  { name: "DeepSeek V3", context: 128_000 },
  { name: "Llama 3.x (70B+)", context: 128_000 },
  { name: "Grok 2 / 3", context: 131_072 },
  { name: "o1 / o3", context: 200_000 },
  { name: "Claude 3.x / Claude 4.x", context: 200_000 },
  { name: "Gemini 1.5 Pro", context: 1_048_576 },
  { name: "Gemini 2.0 Flash", context: 1_048_576 },
  { name: "Gemini 2.5 Pro", context: 1_048_576 },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function fmtNum(n: number): string {
  return n.toLocaleString();
}

function fmtBytes(b: number): string {
  if (b >= 1_048_576) return `${(b / 1_048_576).toFixed(1)} MB`;
  if (b >= 1_024) return `${(b / 1_024).toFixed(1)} KB`;
  return `${b} B`;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------
function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2 group">
      <input
        type="checkbox"
        className="mt-0.5 h-4 w-4 shrink-0 rounded accent-emerald-500"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>
        <span className="text-sm text-zinc-200 group-hover:text-white">{label}</span>
        {hint && <span className="ml-1.5 text-xs text-zinc-500">{hint}</span>}
      </span>
    </label>
  );
}

function CopyButton({ label, text }: { label: string; text: string }) {
  const [copied, setCopied] = useState(false);
  async function doCopy() {
    await writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }
  return (
    <button
      onClick={doCopy}
      className="rounded border border-zinc-600 bg-zinc-800 px-4 py-2 text-sm hover:bg-zinc-700 active:scale-95 transition-all"
    >
      {copied ? "✓ Copied!" : label}
    </button>
  );
}

function StatsBar({ stats }: { stats: PackStats }) {
  return (
    <div className="flex flex-wrap gap-x-6 gap-y-2 rounded border border-zinc-700 bg-zinc-800/50 px-4 py-3 text-sm">
      <span>
        <span className="text-zinc-400">Files </span>
        <span className="font-medium text-zinc-100">{stats.filesIncluded}</span>
        <span className="text-zinc-500"> / {stats.filesTotal}</span>
      </span>
      <span>
        <span className="text-zinc-400">Skipped </span>
        <span className="font-medium text-zinc-100">{stats.filesSkipped}</span>
      </span>
      <span>
        <span className="text-zinc-400">Size </span>
        <span className="font-medium text-zinc-100">{fmtBytes(stats.bytesTotal)}</span>
      </span>
      {stats.tokensTotal != null && (
        <span>
          <span className="text-zinc-400">Tokens </span>
          <span className="font-medium text-zinc-100">{fmtNum(stats.tokensTotal)}</span>
        </span>
      )}
      {stats.secretsFound > 0 && (
        <span className="font-medium text-amber-400">
          ⚠ {stats.secretsFound} secret{stats.secretsFound !== 1 ? "s" : ""} detected
        </span>
      )}
      <span>
        <span className="text-zinc-400">Time </span>
        <span className="font-medium text-zinc-100">{stats.durationMs}ms</span>
      </span>
    </div>
  );
}

function AiContextTable({ tokens }: { tokens: number }) {
  return (
    <div className="overflow-hidden rounded border border-zinc-700">
      <div className="border-b border-zinc-700 bg-zinc-800 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">
        AI Context Window Compatibility — {fmtNum(tokens)} tokens packed
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-zinc-700 text-xs text-zinc-500">
            <th className="px-3 py-2 text-left font-normal">Model</th>
            <th className="px-3 py-2 text-right font-normal">Context limit</th>
            <th className="px-3 py-2 text-center font-normal">Fits?</th>
          </tr>
        </thead>
        <tbody>
          {AI_MODELS.map((m) => {
            const fits = tokens <= m.context;
            const pct = Math.min(100, Math.round((tokens / m.context) * 100));
            return (
              <tr key={m.name} className="border-b border-zinc-800 last:border-0">
                <td className="px-3 py-2 text-zinc-200">{m.name}</td>
                <td className="px-3 py-2 text-right text-zinc-400">{fmtNum(m.context)}</td>
                <td className="px-3 py-2">
                  <div className="flex items-center justify-center gap-2">
                    {fits ? (
                      <span className="font-medium text-emerald-400">✓ Yes</span>
                    ) : (
                      <span className="font-medium text-red-400">✗ No</span>
                    )}
                    <span className="text-xs text-zinc-500">({pct}%)</span>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function ProgressLog({ events }: { events: ProgressEvent[] }) {
  const lines: string[] = events
    .map((e) => {
      if (e.kind === "started") return `▶ ${e.target_label}`;
      if (e.kind === "walking") return `  Walking… ${e.files_scanned} files scanned`;
      if (e.kind === "tokenizing") return `  Tokenizing… ${e.progress_pct}%`;
      if (e.kind === "secretScanning") return `  Secret scan… ${e.progress_pct}%`;
      if (e.kind === "compressing") return `  Compressing… ${e.progress_pct}%`;
      if (e.kind === "buildingXml") return `  Building output…`;
      if (e.kind === "secretHit") return `  ⚠ Secret in ${e.path} (line ${e.line})`;
      if (e.kind === "done") return `✓ Done`;
      if (e.kind === "error") return `✗ Error: ${e.message}`;
      return null;
    })
    .filter((l): l is string => l !== null);

  return (
    <div className="rounded border border-zinc-700 bg-zinc-900 px-4 py-3">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">Progress</div>
      <div className="space-y-0.5 font-mono text-xs text-zinc-400">
        {lines.slice(-16).map((l, i) => (
          <div key={i}>{l}</div>
        ))}
      </div>
    </div>
  );
}

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
    options, setOptions,
    status, events,
    setJob, pushEvent, setResult, result,
    reset,
  } = useApp();

  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  async function pickFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      setOptions({ ...options, target: { kind: "folder", value: path } });
    }
  }

  async function runPack() {
    setErrorMsg(null);
    reset();
    const startRes = await commands.packStart(options);
    if (startRes.status !== "ok") {
      setErrorMsg(startRes.error.message);
      return;
    }
    const jobId = startRes.data;
    setJob(jobId);
    const unlisten = await subscribePackProgress(jobId, (e) => {
      pushEvent(e);
      if (e.kind === "done") {
        (async () => {
          const r = await commands.packGetResult(jobId);
          if (r.status === "ok") setResult(r.data);
          else setErrorMsg(r.error.message);
          await unlisten();
        })();
      }
      if (e.kind === "error" && e.fatal) {
        setErrorMsg(e.message);
        unlisten();
      }
    });
  }

  const targetVal = options.target.kind === "folder" ? options.target.value : "";
  const isRunning = status === "running";
  const isDone = status === "done";

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <div className="mx-auto max-w-2xl space-y-6 p-6">

        {/* ── Header ── */}
        <div>
          <h1 className="text-2xl font-bold tracking-tight">ProjectPacker</h1>
          <p className="mt-1 text-sm text-zinc-400">
            Pack a codebase into a single file for AI consumption.
          </p>
        </div>

        {/* ── Target folder ── */}
        <section className="space-y-2">
          <label className="block text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Target Folder
          </label>
          <div className="flex gap-2">
            <input
              className="flex-1 rounded border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm focus:border-zinc-500 focus:outline-none"
              value={targetVal}
              placeholder="/path/to/project"
              onChange={(e) =>
                setOptions({ ...options, target: { kind: "folder", value: e.target.value } })
              }
            />
            <button
              className="rounded border border-zinc-600 bg-zinc-700 px-3 py-2 text-sm hover:bg-zinc-600"
              onClick={pickFolder}
            >
              Browse…
            </button>
          </div>
        </section>

        {/* ── Goal ── */}
        <section className="space-y-2">
          <label className="block text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Goal / Task Description
          </label>
          <textarea
            className="h-20 w-full resize-none rounded border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm focus:border-zinc-500 focus:outline-none"
            value={options.goal}
            placeholder="Describe what you want to build or fix…"
            onChange={(e) => setOptions({ ...options, goal: e.target.value })}
          />
        </section>

        {/* ── Options ── */}
        <section className="space-y-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-zinc-500">Options</div>

          <div className="grid grid-cols-2 gap-x-8 gap-y-3">
            <Toggle
              label="Compress to skeleton"
              hint="strips bodies, keeps signatures"
              checked={options.compress}
              onChange={(v) => setOptions({ ...options, compress: v })}
            />
            <Toggle
              label="Remove comments"
              hint="tree-sitter: Rust/Py/JS/TS"
              checked={options.removeComments}
              onChange={(v) => setOptions({ ...options, removeComments: v })}
            />
            <Toggle
              label="Respect .gitignore"
              checked={options.respectGitignore}
              onChange={(v) => setOptions({ ...options, respectGitignore: v })}
            />
            <Toggle
              label="Scan for secrets"
              checked={options.secretScan}
              onChange={(v) => setOptions({ ...options, secretScan: v })}
            />
            <Toggle
              label="Count tokens"
              hint="gpt-4o-mini tokenizer"
              checked={options.countTokens}
              onChange={(v) => setOptions({ ...options, countTokens: v })}
            />
          </div>

          <div className="flex flex-wrap items-center gap-6">
            <div className="flex items-center gap-2">
              <label className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                Output Format
              </label>
              <select
                className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-sm focus:border-zinc-500 focus:outline-none"
                value={options.format}
                onChange={(e) =>
                  setOptions({ ...options, format: e.target.value as PackFormat })
                }
              >
                {(Object.keys(FORMAT_LABELS) as PackFormat[]).map((f) => (
                  <option key={f} value={f}>
                    {FORMAT_LABELS[f]}
                  </option>
                ))}
              </select>
            </div>

            <div className="flex items-center gap-2">
              <label className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                Max File Size
              </label>
              <input
                type="number"
                min={1}
                max={102_400}
                className="w-20 rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-sm focus:border-zinc-500 focus:outline-none"
                value={options.maxFileSizeKb}
                onChange={(e) =>
                  setOptions({ ...options, maxFileSizeKb: Number(e.target.value) })
                }
              />
              <span className="text-xs text-zinc-500">KB</span>
            </div>
          </div>
        </section>

        {/* ── Pack button ── */}
        <button
          className="w-full rounded bg-emerald-700 py-3 text-sm font-semibold transition-colors hover:bg-emerald-600 disabled:cursor-not-allowed disabled:opacity-40"
          onClick={runPack}
          disabled={isRunning || !targetVal}
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
            <div className="text-xs font-semibold uppercase tracking-wide text-zinc-500">Result</div>

            <StatsBar stats={result.stats} />

            <div className="flex flex-wrap gap-2">
              <CopyButton
                label={COPY_BUTTON_LABELS[options.format]}
                text={result.output}
              />
              <CopyButton label="Copy Claude Code Prompt" text={result.claudeCodePrompt} />
              <button
                className="rounded border border-zinc-600 bg-zinc-800 px-4 py-2 text-sm hover:bg-zinc-700"
                onClick={() => reset()}
              >
                New Pack
              </button>
            </div>

            {result.warnings.length > 0 && (
              <div className="rounded border border-amber-700 bg-amber-950/30 px-4 py-3 text-sm">
                <div className="mb-1 font-semibold text-amber-400">Warnings</div>
                {result.warnings.map((w, i) => (
                  <div key={i} className="text-xs text-amber-300">
                    {w.message}
                  </div>
                ))}
              </div>
            )}

            {result.stats.tokensTotal != null && (
              <AiContextTable tokens={result.stats.tokensTotal} />
            )}
          </div>
        )}

      </div>
    </div>
  );
}
