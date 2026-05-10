import * as m from "framer-motion/m";
import { type KeyboardEvent, useId } from "react";

interface ToggleProps {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}

/**
 * Card-row switch: a polished toggle that feels at home in a dense options
 * panel. The visual track + label sit inside one rounded card so a single
 * click anywhere on the row flips the value (the `<label>` association
 * carries the click for keyboard + assistive tech users too).
 *
 * Accessibility:
 *  - sr-only native `<input type="checkbox">` for form-association +
 *    screen-reader semantics.
 *  - Visible track carries `role="switch"`, `aria-checked`, `aria-label`
 *    and a Space/Enter keydown handler so the keyboard flips the switch
 *    even when the input isn't focused (the visible track is what
 *    receives focus via tabIndex=0).
 *  - 2px emerald `focus-visible` ring inherits from the global rule in
 *    `globals.css`; we don't redeclare it per-component.
 *  - Thumb x-offset uses a framer-motion spring so the slide reads as
 *    physical rather than as a CSS transition. The thumb is `aria-hidden`
 *    because the role-switch already conveys state.
 */
export function Toggle({ label, hint, checked, onChange }: ToggleProps) {
  const inputId = useId();

  function onTrackKeyDown(e: KeyboardEvent<HTMLSpanElement>) {
    // Space is the WAI-ARIA standard for switch activation; Enter is a
    // common-sense addition (some keyboards lack a true space key, e.g.
    // chorded ergonomic boards).
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      onChange(!checked);
    }
  }

  return (
    <label
      htmlFor={inputId}
      className={`group flex cursor-pointer items-center justify-between gap-4 rounded-xl border px-4 py-3 transition-all duration-200 ${
        checked
          ? "border-emerald-500/40 bg-emerald-500/10 shadow-[0_0_24px_rgba(16,185,129,0.08)]"
          : "border-zinc-700/60 bg-zinc-900/40 hover:border-zinc-600 hover:bg-zinc-800/50"
      }`}
    >
      <span className="min-w-0">
        <span
          className={`block text-sm font-medium transition-colors ${
            checked
              ? "text-emerald-100"
              : "text-zinc-200 group-hover:text-white"
          }`}
        >
          {label}
        </span>
        {hint && (
          <span className="mt-0.5 block text-xs leading-relaxed text-zinc-500">
            {hint}
          </span>
        )}
      </span>
      <span className="relative shrink-0">
        <input
          id={inputId}
          type="checkbox"
          className="sr-only"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
        />
        <span
          role="switch"
          aria-checked={checked}
          aria-label={label}
          tabIndex={0}
          onKeyDown={onTrackKeyDown}
          onClick={(e) => {
            // Click bubbles from the visible track AND from the label;
            // suppress the visible-track path so we don't toggle twice.
            e.preventDefault();
            onChange(!checked);
          }}
          className={`block h-7 w-12 rounded-full border transition-all duration-200 outline-none ${
            checked
              ? "border-emerald-400 bg-emerald-500"
              : "border-zinc-600 bg-zinc-700 group-hover:border-zinc-500"
          }`}
        >
          <m.span
            className="block h-5 w-5 rounded-full bg-white shadow-md"
            animate={{ x: checked ? 22 : 4, y: 3 }}
            transition={{ type: "spring", stiffness: 520, damping: 32 }}
            aria-hidden="true"
          />
        </span>
      </span>
    </label>
  );
}
