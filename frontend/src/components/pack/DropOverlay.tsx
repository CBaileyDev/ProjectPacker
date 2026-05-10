import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { useEffect, useState } from "react";
import { AlertIcon, FolderOpenIcon, KeyboardIcon } from "./icons";

type DropState = "idle" | "valid" | "invalid";

interface DropOverlayProps {
  visible: boolean;
  dropState?: DropState;
}

/**
 * Live `prefers-reduced-motion` listener. We can't reuse the static
 * `prefersReducedMotion` from `lib/motion.ts` here because that snapshot
 * is captured at module-load time — DropOverlay needs to react when the
 * user toggles the OS setting mid-session (or starts a screen recorder
 * that flips the flag). A lightweight `useState` + `addEventListener`
 * keeps the rest of the app's static reads cheap while giving this
 * single component live reactivity.
 */
function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  });

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mql = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = (e: MediaQueryListEvent) => setReduced(e.matches);
    // `addEventListener` form is the cross-browser current API; the
    // legacy `addListener` shim isn't worth supporting here.
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  return reduced;
}

/**
 * Full-screen overlay shown while a drag-drop is in flight.
 *
 * Two visual states drive the look-and-feel:
 *  - `'valid'`: emerald theme, pulsing folder icon, "Drop folder to pack"
 *  - `'invalid'`: amber theme, alert icon, "Invalid drop — folders only"
 *
 * The component is `pointer-events-none` end-to-end so the underlying
 * webview still receives the OS-level drop event; the overlay is purely
 * visual.
 *
 * Backwards-compat: `dropState` defaults to `'valid'` so legacy callers
 * passing only `visible` still get the expected emerald drop affordance.
 *
 * Reduced motion: the continuous CSS-keyframe pulse ring is gated, the
 * y-bounce is replaced with a static frame, and AnimatePresence
 * transitions collapse to ~0ms.
 */
export function DropOverlay({
  visible,
  dropState = "valid",
}: DropOverlayProps) {
  const reducedMotion = useReducedMotion();
  // When idle and not visible, render nothing — no DOM, no listeners,
  // no flash on remount during options-panel re-renders.
  if (dropState === "idle" && !visible) return null;

  const isInvalid = dropState === "invalid";
  const accent = isInvalid ? "amber" : "emerald";

  // Pre-compose the variant strings so each Tailwind class is a literal
  // somewhere in source — the JIT scanner can't see template-string
  // computed classnames.
  const cardBorderCls = isInvalid
    ? "border-amber-400/60"
    : "border-emerald-400/60";
  const titleCls = isInvalid ? "text-amber-300" : "text-emerald-300";
  const subtitleCls = isInvalid ? "text-amber-400/60" : "text-emerald-400/60";
  const iconCls = isInvalid ? "text-amber-400" : "text-emerald-400";
  const backdropCls = isInvalid ? "bg-amber-500/8" : "bg-emerald-500/8";

  const title = isInvalid
    ? "Invalid drop — folders only"
    : "Drop folder to pack";
  const subtitle = isInvalid
    ? "ProjectPacker only accepts directories"
    : "Release to select this folder";

  return (
    <AnimatePresence>
      {(visible || dropState === "invalid") && (
        <m.div
          // pointer-events-none lets the underlying webview still receive
          // the drop event; the overlay is purely visual.
          aria-hidden="true"
          className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: reducedMotion ? 0.05 : 0.2 }}
          data-state={dropState}
          data-accent={accent}
        >
          <m.div
            className={`absolute inset-0 ${backdropCls} backdrop-blur-sm`}
            aria-hidden="true"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: reducedMotion ? 0.05 : 0.2 }}
          />
          <m.div
            className="relative z-10"
            initial={{ scale: reducedMotion ? 1 : 0.9, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: reducedMotion ? 1 : 0.9, opacity: 0 }}
            transition={
              reducedMotion
                ? { duration: 0.05 }
                : { type: "spring", stiffness: 400, damping: 28 }
            }
          >
            <div
              // The pulse keyframe is defined in `globals.css`. Gate the
              // continuous animation behind reducedMotion so AT users
              // don't get a 1.6s loop forever.
              style={
                reducedMotion || isInvalid
                  ? undefined
                  : {
                      animation: "drop-overlay-pulse 1.8s ease-in-out infinite",
                      willChange: "transform, opacity",
                    }
              }
              className={`rounded-2xl border-2 border-dashed ${cardBorderCls} bg-zinc-900/95 px-12 py-10 text-center shadow-2xl`}
            >
              {isInvalid ? (
                <div aria-hidden="true">
                  <AlertIcon size={40} className={`mx-auto mb-3 ${iconCls}`} />
                </div>
              ) : (
                <m.div
                  aria-hidden="true"
                  animate={reducedMotion ? { y: 0 } : { y: [0, -4, 0] }}
                  transition={
                    reducedMotion
                      ? { duration: 0 }
                      : {
                          duration: 2,
                          repeat: Infinity,
                          ease: "easeInOut",
                        }
                  }
                >
                  <FolderOpenIcon
                    size={40}
                    className={`mx-auto mb-3 ${iconCls}`}
                  />
                </m.div>
              )}
              <div className={`text-lg font-semibold ${titleCls}`}>{title}</div>
              <div className={`mt-1 text-sm ${subtitleCls}`}>{subtitle}</div>

              <div className="mt-5 flex items-center justify-center gap-2 text-xs text-zinc-500">
                <KeyboardIcon size={12} className="opacity-70" />
                <span>Press Tab + Space to browse instead</span>
              </div>
            </div>
          </m.div>
        </m.div>
      )}
    </AnimatePresence>
  );
}
