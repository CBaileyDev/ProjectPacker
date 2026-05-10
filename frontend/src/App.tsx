import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useState } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { AlertIcon, CheckIcon, XIcon } from "./components/pack/icons";
import { useToast } from "./lib/toast";
import Pack from "./routes/Pack";

/**
 * Sync the document `<html>` `.dark` class with the OS-level
 * `prefers-color-scheme` media query.
 *
 *  - Reads the initial value once on mount, applies it synchronously
 *    so the first paint already matches the OS preference (no flash
 *    of light theme on a dark-mode user).
 *  - Subscribes to the change event so flipping the OS theme during
 *    the session updates the class without requiring a reload.
 *  - Cleans up the listener on unmount; not strictly necessary at
 *    the App root (which never unmounts in practice) but harmless
 *    and a good template for components that copy this pattern.
 */
function useSystemTheme(): void {
  useEffect(() => {
    if (typeof window === "undefined") return;
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const root = document.documentElement;

    function apply(matches: boolean) {
      // toggle() with the explicit boolean is idempotent — calling it
      // when the class is already correct is a no-op.
      root.classList.toggle("dark", matches);
    }

    apply(mql.matches);
    const onChange = (e: MediaQueryListEvent) => apply(e.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);
}

const TOAST_KIND_STYLES: Record<
  "info" | "success" | "error",
  { bg: string; text: string; ring: string; Icon: React.FC<{ size?: number; className?: string }> }
> = {
  info: {
    bg: "bg-zinc-800/95",
    text: "text-zinc-100",
    ring: "ring-zinc-700",
    Icon: AlertIcon,
  },
  success: {
    bg: "bg-emerald-950/90",
    text: "text-emerald-200",
    ring: "ring-emerald-700/60",
    Icon: CheckIcon,
  },
  error: {
    bg: "bg-red-950/90",
    text: "text-red-200",
    ring: "ring-red-700/60",
    Icon: XIcon,
  },
};

/**
 * Stack of live toasts, rendered at the root so any component that
 * calls `useToast().showToast(...)` produces a visible message.
 *
 * Animated slide-in-from-bottom; clicking the toast dismisses it
 * without waiting for the auto-dismiss timer.
 */
function Toaster() {
  const { toasts, dismissToast } = useToast();
  return (
    <div
      // pointer-events-none on the wrapper so the toast stack doesn't
      // capture clicks meant for the underlying UI; individual toast
      // children re-enable pointer events for their own click handlers.
      aria-live="polite"
      aria-atomic="false"
      className="pointer-events-none fixed inset-x-0 bottom-0 z-[60] flex flex-col items-center gap-2 px-4 pb-4"
    >
      <AnimatePresence>
        {toasts.map((t) => {
          const style = TOAST_KIND_STYLES[t.kind] ?? TOAST_KIND_STYLES.info;
          const Icon = style.Icon;
          return (
            <motion.button
              type="button"
              key={t.id}
              onClick={() => dismissToast(t.id)}
              className={`pointer-events-auto flex max-w-md items-center gap-2.5 rounded-lg px-4 py-2.5 text-sm shadow-lg ring-1 backdrop-blur ${style.bg} ${style.text} ${style.ring}`}
              initial={{ opacity: 0, y: 16, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 16, scale: 0.96 }}
              transition={{ type: "spring", stiffness: 350, damping: 28 }}
            >
              <Icon size={14} />
              <span>{t.message}</span>
            </motion.button>
          );
        })}
      </AnimatePresence>
    </div>
  );
}

/**
 * Error boundary fallback. Shows the error message immediately and the
 * full stack trace inside a `<details>` element — collapsed by default
 * so a user-facing crash isn't an opaque wall of frames.
 */
function ErrorFallback({
  error,
  resetErrorBoundary,
}: {
  error: Error;
  resetErrorBoundary: () => void;
}) {
  // Pull the stack defensively — Error.stack is non-standard and may be
  // missing in some runtimes (we've seen Tauri's WebKit drop it under
  // very specific GC conditions).
  const stack = typeof error?.stack === "string" ? error.stack : null;
  const [showStack, setShowStack] = useState(false);

  return (
    <div className="min-h-screen p-6 text-sm text-zinc-200">
      <div className="mx-auto max-w-2xl space-y-4">
        <div className="flex items-center gap-2 text-red-400">
          <AlertIcon size={18} />
          <h2 className="text-lg font-semibold">Something went wrong</h2>
        </div>
        <pre className="overflow-auto rounded-lg border border-red-700/40 bg-red-950/40 p-3 text-xs text-red-200">
          {error.message}
        </pre>
        {stack && (
          <details
            open={showStack}
            onToggle={(e) => setShowStack(e.currentTarget.open)}
            className="rounded-lg border border-zinc-700/60 bg-zinc-900/60"
          >
            <summary className="cursor-pointer select-none px-3 py-2 text-xs font-semibold text-zinc-400 hover:text-zinc-200">
              Stack trace
            </summary>
            <pre className="overflow-auto border-t border-zinc-700/60 p-3 text-[11px] leading-relaxed text-zinc-400">
              {stack}
            </pre>
          </details>
        )}
        <button
          type="button"
          onClick={resetErrorBoundary}
          className="rounded-lg border border-zinc-600 bg-zinc-800 px-4 py-2 text-sm text-zinc-200 hover:bg-zinc-700 transition-colors"
        >
          Reload
        </button>
      </div>
    </div>
  );
}

export default function App() {
  useSystemTheme();
  return (
    <ErrorBoundary FallbackComponent={ErrorFallback}>
      <Pack />
      <Toaster />
    </ErrorBoundary>
  );
}
