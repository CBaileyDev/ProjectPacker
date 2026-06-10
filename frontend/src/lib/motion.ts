import type { Variants } from "framer-motion";

export const prefersReducedMotion =
  typeof window !== "undefined" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export const reduced = { duration: 0 };

// ── Easing curves ───────────────────────────────────────────────────────────
// `as const` so they keep their tuple shape — Framer's transition.ease typing
// requires a fixed-length number tuple, not a `number[]`.
export const easeOutCustom = [0.22, 1, 0.36, 1] as const;

// ── Spring presets ──────────────────────────────────────────────────────────
// Stiffness/damping tuned for 60fps perception; not auto-derived from a base
// spring so each preset can be hand-fitted to its use case.
export const springQuick = {
  type: "spring" as const,
  stiffness: 500,
  damping: 25,
};

// ── Entrance variants ───────────────────────────────────────────────────────
// Each variant gates its motion behind `prefersReducedMotion`. We can't use
// the parent-level `MotionConfig` reducedMotion="user" flag because some of
// our transitions blend opacity AND y/scale — letting Framer strip just the
// transform leaves a sub-second flash where the element is opacity:0 with
// duration:0, which still flickers. Removing the offsets entirely is cleaner.
export const fadeUp: Variants = {
  hidden: { opacity: 0, y: prefersReducedMotion ? 0 : 12 },
  visible: {
    opacity: 1,
    y: 0,
    transition: prefersReducedMotion
      ? reduced
      : { duration: 0.35, ease: easeOutCustom },
  },
};

export const slideInUp: Variants = {
  hidden: { opacity: 0, y: prefersReducedMotion ? 0 : 20 },
  visible: {
    opacity: 1,
    y: 0,
    transition: prefersReducedMotion
      ? reduced
      : { duration: 0.35, ease: easeOutCustom },
  },
};

// ── Stagger ─────────────────────────────────────────────────────────────────
export const staggerContainer: Variants = {
  hidden: {},
  visible: {
    transition: { staggerChildren: 0.04, delayChildren: 0.05 },
  },
};

// ── Tap/hover ───────────────────────────────────────────────────────────────
export const springButton = {
  scale: 0.97,
  transition: springQuick,
};

// ── Bar height transition ──────────────────────────────────────────────────
// Used by progress bars and phase breakdowns where we tween width/height.
// `delay: 0.1` lets parent stagger settle before the bar fills. Caller passes
// reducedMotion so this can be wired in non-React contexts (e.g. canvas).
export function barHeightTransition(reducedMotion: boolean) {
  return {
    duration: reducedMotion ? 0 : 0.6,
    ease: easeOutCustom,
    delay: 0.1,
  };
}
