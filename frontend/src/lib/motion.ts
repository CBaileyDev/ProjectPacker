import type { Variants } from "framer-motion";

export const prefersReducedMotion =
  typeof window !== "undefined" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export const reduced = { duration: 0 };

// ── Easing curves ───────────────────────────────────────────────────────────
// `as const` so they keep their tuple shape — Framer's transition.ease typing
// requires a fixed-length number tuple, not a `number[]`.
export const easeOutCustom = [0.22, 1, 0.36, 1] as const;
export const easeInOut = [0.42, 0, 0.58, 1] as const;
export const easeSpringLike = [0.2, 0.8, 0.4, 1] as const;

// ── Spring presets ──────────────────────────────────────────────────────────
// Stiffness/damping tuned for 60fps perception; not auto-derived from a base
// spring so each preset can be hand-fitted to its use case.
export const springQuick = {
  type: "spring" as const,
  stiffness: 500,
  damping: 25,
};
export const springDefault = {
  type: "spring" as const,
  stiffness: 300,
  damping: 25,
};
export const springBouncy = {
  type: "spring" as const,
  stiffness: 400,
  damping: 15,
};
export const springSoft = {
  type: "spring" as const,
  stiffness: 200,
  damping: 30,
};

// `whileHover` config used for interactive surfaces. Springy lift kept subtle
// because anything bouncier reads as toy-like in a dense form layout.
export const springHover = {
  scale: 1.02,
  transition: springQuick,
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

export const fadeDown: Variants = {
  hidden: { opacity: 0, y: prefersReducedMotion ? 0 : -12 },
  visible: {
    opacity: 1,
    y: 0,
    transition: prefersReducedMotion
      ? reduced
      : { duration: 0.35, ease: easeOutCustom },
  },
};

export const fadeIn: Variants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: prefersReducedMotion
      ? reduced
      : { duration: 0.3, ease: "easeOut" },
  },
};

export const scaleIn: Variants = {
  hidden: { opacity: 0, scale: prefersReducedMotion ? 1 : 0.96 },
  visible: {
    opacity: 1,
    scale: 1,
    transition: prefersReducedMotion
      ? reduced
      : { duration: 0.3, ease: easeOutCustom },
  },
};

export const scaleInBouncy: Variants = {
  hidden: { opacity: 0, scale: prefersReducedMotion ? 1 : 0.85 },
  visible: {
    opacity: 1,
    scale: 1,
    transition: prefersReducedMotion ? reduced : springBouncy,
  },
};

export const slideInLeft: Variants = {
  hidden: { opacity: 0, x: prefersReducedMotion ? 0 : -20 },
  visible: {
    opacity: 1,
    x: 0,
    transition: prefersReducedMotion
      ? reduced
      : { duration: 0.35, ease: easeOutCustom },
  },
};

export const slideInRight: Variants = {
  hidden: { opacity: 0, x: prefersReducedMotion ? 0 : 20 },
  visible: {
    opacity: 1,
    x: 0,
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

export function staggerContainerWith(
  staggerChildren: number,
  delayChildren = 0,
): Variants {
  return {
    hidden: {},
    visible: {
      transition: prefersReducedMotion
        ? { staggerChildren: 0, delayChildren: 0 }
        : { staggerChildren, delayChildren },
    },
  };
}

// ── Feedback variants ───────────────────────────────────────────────────────
// `pulse` for momentary attention (toast appearance, copy-success), `shake`
// for input-validation error nudges. Both collapse to opacity-only when
// reduced motion is requested.
export const pulse: Variants = {
  initial: { scale: 1 },
  animate: prefersReducedMotion
    ? { scale: 1 }
    : {
        scale: [1, 1.04, 1],
        transition: { duration: 0.6, ease: easeOutCustom },
      },
};

export const shake: Variants = {
  initial: { x: 0 },
  animate: prefersReducedMotion
    ? { x: 0 }
    : {
        x: [0, -6, 6, -4, 4, 0],
        transition: { duration: 0.4, ease: easeInOut },
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
