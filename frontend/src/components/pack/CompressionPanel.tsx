import { useState } from "react";
import type { PackOptions } from "../../bindings";
import { useLastStats, usePackOptions } from "../../lib/store";
import { TransformRow } from "./TransformRow";

/**
 * One compression-toggle declaration. The store maps each `key` to a
 * boolean inside `PackOptions` (camelCase, since specta emits camelCase
 * for the wire types); `id` is the snake_case identifier the Rust
 * pipeline emits in `TransformReport.id` and the `transformDone`
 * progress event. Keeping the two separate avoids a fragile string
 * transform at the lookup site.
 */
interface TransformSpec {
  key: keyof Pick<
    PackOptions,
    | "dedupFiles"
    | "trimTrailingWs"
    | "collapseBlankLines"
    | "normalizeLineEndings"
    | "collapseLockfiles"
    | "collapseMinified"
    | "markGenerated"
    | "compress"
    | "removeComments"
    | "elideTypeOnlyExports"
  >;
  id: string;
  label: string;
  description: string;
}

const LOSSLESS: TransformSpec[] = [
  {
    key: "dedupFiles",
    id: "dedup_files",
    label: "Dedup duplicate files",
    description:
      "Identical files (LICENSE copies, vendored libs) become a content-pointer.",
  },
  {
    key: "trimTrailingWs",
    id: "trim_trailing_ws",
    label: "Trim trailing whitespace",
    description: "Strips trailing spaces and tabs from every line.",
  },
  {
    key: "collapseBlankLines",
    id: "collapse_blank_lines",
    label: "Collapse blank lines",
    description: "Runs of 3+ blank lines collapse to 2.",
  },
  {
    key: "normalizeLineEndings",
    id: "normalize_line_endings",
    label: "Normalize line endings (CRLF → LF)",
    description: "CRLF and lone CR are converted to LF.",
  },
];

const SEMANTIC: TransformSpec[] = [
  {
    key: "collapseLockfiles",
    id: "collapse_lockfiles",
    label: "Collapse lockfiles",
    description:
      "package-lock.json, pnpm-lock.yaml, Cargo.lock, etc.: keep head/tail + marker.",
  },
  {
    key: "collapseMinified",
    id: "collapse_minified",
    label: "Collapse minified bundles",
    description:
      "Single-line or extreme-variance bundles: replace body with a marker.",
  },
  {
    key: "markGenerated",
    id: "mark_generated",
    label: "Mark generated files",
    description:
      "Files with @generated banners or *.pb.go/.gen.ts patterns: suppress body.",
  },
];

const LOSSY: TransformSpec[] = [
  {
    key: "compress",
    id: "compress",
    label: "Skeleton-compress functions",
    description:
      "Replace function/class/method bodies with a skeleton (rs/py/js/ts).",
  },
  {
    key: "removeComments",
    id: "remove_comments",
    label: "Strip comments",
    description: "Remove comments from rs/py/js/ts files.",
  },
  {
    key: "elideTypeOnlyExports",
    id: "elide_type_only_exports",
    label: "Elide TypeScript type-only re-exports",
    description: "Strip 'export type { … } from …' lines.",
  },
];

const ALL_SPECS: TransformSpec[] = [...LOSSLESS, ...SEMANTIC, ...LOSSY];

function formatBytes(n: number): string {
  if (n === 0) return "0 B";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

/**
 * Collapsible disclosure that bundles all 10 compression toggles into one
 * unit. Three grouped sections (lossless / semantic / code shaping) match
 * the Rust-side categorization; each group has a bulk-toggle chip showing
 * "all on" / "all off" / "n of m on". When a pack has been run, per-row
 * `bytesSaved` is sourced from `lastStats.transforms` by id so the user
 * sees exactly which transforms paid off.
 *
 * The component reads from `usePackOptions` (so a toggle flip propagates
 * via the same `patchOptions` action the legacy inline toggles used) and
 * `useLastStats` (granular subscription — unrelated re-renders don't
 * rebuild the 10-row list). Closed by default so the toggle wall doesn't
 * dominate the Packer tab.
 */
export function CompressionPanel() {
  const [open, setOpen] = useState(false);
  const { options, patchOptions } = usePackOptions();
  const stats = useLastStats();
  const transforms = stats?.transforms ?? [];

  const reportFor = (id: string) => transforms.find((t) => t.id === id);

  const enabledCount = ALL_SPECS.filter((spec) => options[spec.key]).length;
  const totalSaved = transforms.reduce((acc, t) => acc + t.bytesSaved, 0);

  function renderGroup(title: string, caption: string, specs: TransformSpec[]) {
    const allOn = specs.every((s) => options[s.key]);
    const allOff = specs.every((s) => !options[s.key]);
    const someOn = specs.filter((s) => options[s.key]).length;
    const chipLabel = allOn
      ? "✓ all on"
      : allOff
        ? "✗ all off"
        : `∼ ${someOn} of ${specs.length} on`;
    const toggleAll = () => {
      const next = !allOn;
      const patch: Partial<PackOptions> = {};
      for (const s of specs) {
        patch[s.key] = next;
      }
      patchOptions(patch);
    };
    return (
      <div key={title} className="mt-3">
        <div className="mb-1 flex items-center justify-between px-2">
          <div className="min-w-0">
            <span className="text-[13px] font-semibold tracking-wide text-zinc-300">
              {title}
            </span>
            <span className="ml-2 text-[11px] text-zinc-500">{caption}</span>
          </div>
          <button
            type="button"
            className="cursor-pointer text-[11px] text-zinc-500 transition-colors hover:text-zinc-200"
            onClick={toggleAll}
          >
            {chipLabel}
          </button>
        </div>
        <div>
          {specs.map((spec) => {
            const report = reportFor(spec.id);
            return (
              <TransformRow
                key={spec.key}
                label={spec.label}
                description={spec.description}
                checked={Boolean(options[spec.key])}
                onToggle={(v) =>
                  patchOptions({ [spec.key]: v } as Partial<PackOptions>)
                }
                bytesSaved={report?.bytesSaved}
                noEligibleFiles={Boolean(report) && report?.filesTouched === 0}
              />
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-xl border border-zinc-700/50 bg-zinc-800/30">
      <button
        type="button"
        className="flex w-full items-center justify-between gap-3 px-3 py-2.5 transition-colors hover:bg-zinc-800/50"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
      >
        <span className="flex items-center gap-2 text-sm font-medium text-zinc-200">
          <span aria-hidden="true" className="inline-block w-3 text-zinc-500">
            {open ? "▾" : "▸"}
          </span>
          Compression
          <span className="ml-1 text-xs font-normal text-zinc-500">
            {enabledCount} of {ALL_SPECS.length} enabled
          </span>
        </span>
        <span className="nums text-xs text-transform-savings">
          {transforms.length > 0
            ? `Last run: ${formatBytes(totalSaved)} saved`
            : ""}
        </span>
      </button>
      {open && (
        <div className="border-t border-zinc-700/40 px-2 pb-3">
          {renderGroup("LOSSLESS", "applied by default", LOSSLESS)}
          {renderGroup("SEMANTIC", "opt-in, no information loss", SEMANTIC)}
          {renderGroup("CODE SHAPING", "opt-in, modifies code (lossy)", LOSSY)}
        </div>
      )}
    </div>
  );
}
