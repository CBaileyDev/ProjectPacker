import { type KeyboardEvent, useId } from "react";

export interface TransformRowProps {
  label: string;
  description: string;
  checked: boolean;
  onToggle: (value: boolean) => void;
  /** Bytes saved by this transform on the last pack, or `undefined` if not
   * run yet. */
  bytesSaved?: number;
  /** True if this transform ran but had zero eligible files (e.g. no
   * lockfiles in the project). Rendered as "n/a — no eligible files"
   * rather than "0 B saved" so the user understands the difference. */
  noEligibleFiles?: boolean;
}

function formatBytes(n: number): string {
  if (n === 0) return "0 B";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

/**
 * One row inside the Compression panel: compact toggle + label + info
 * tooltip + bytes-saved chip. Visually dense compared to the card-row
 * {@link import("./Toggle").Toggle} — that one is built for the
 * options grid and is too tall to stack 10 of into a disclosure.
 *
 * Accessibility: the toggle is a `role="switch"` with `aria-checked` and
 * Space/Enter activation. The description is exposed as a `title` (native
 * tooltip) and as `aria-describedby` so a screen-reader announces it
 * alongside the switch.
 */
export function TransformRow({
  label,
  description,
  checked,
  onToggle,
  bytesSaved,
  noEligibleFiles,
}: TransformRowProps) {
  const descId = useId();
  const savings =
    bytesSaved === undefined
      ? "—"
      : noEligibleFiles
        ? "n/a — no eligible files"
        : `${formatBytes(bytesSaved)} saved`;

  function onKey(e: KeyboardEvent<HTMLButtonElement>) {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      onToggle(!checked);
    }
  }

  return (
    <div className="flex items-center justify-between gap-3 rounded-md px-2 py-1.5 transition-colors hover:bg-zinc-800/40">
      <div className="flex min-w-0 items-center gap-3">
        <button
          type="button"
          role="switch"
          aria-checked={checked}
          aria-label={label}
          aria-describedby={descId}
          onClick={() => onToggle(!checked)}
          onKeyDown={onKey}
          className={`relative inline-flex h-4 w-7 shrink-0 items-center rounded-full border transition-colors duration-150 outline-none ${
            checked
              ? "border-emerald-400 bg-emerald-500"
              : "border-zinc-600 bg-zinc-700 hover:border-zinc-500"
          }`}
        >
          <span
            aria-hidden="true"
            className={`block h-3 w-3 rounded-full bg-white shadow transition-transform duration-150 ${
              checked ? "translate-x-3.5" : "translate-x-0.5"
            }`}
          />
        </button>
        <span className="truncate text-sm font-medium text-zinc-200">
          {label}
        </span>
        {/* The (i) marker is purely visual — the description is already
            associated with the switch via `aria-describedby`, which a
            screen-reader announces when focus lands on the toggle. The
            native `title` covers mouse-hover tooltips for sighted users. */}
        <span
          id={descId}
          className="cursor-help text-xs text-zinc-500 select-none"
          title={description}
        >
          (i)
        </span>
      </div>
      <div className="nums shrink-0 text-xs whitespace-nowrap text-transform-savings">
        {savings}
      </div>
    </div>
  );
}
