import { listen } from "@tauri-apps/api/event";
import { stat } from "@tauri-apps/plugin-fs";
import {
  AnimatePresence,
  domAnimation,
  LazyMotion,
  MotionConfig,
} from "framer-motion";
import * as m from "framer-motion/m";
import type { LucideProps } from "lucide-react";
import { type ComponentType, useEffect, useState } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { DropOverlay } from "./components/pack/DropOverlay";
import { GithubConnector } from "./components/pack/GithubConnector";
import { AlertIcon, CheckIcon, XIcon } from "./components/pack/icons";
import { Settings } from "./components/pack/Settings";
import { Sheet } from "./components/shell/Sheet";
import { TitleBar } from "./components/shell/TitleBar";
import { PackJobProvider, usePackJobContext } from "./lib/pack-job-context";
import { useApp } from "./lib/store";
import { useToast } from "./lib/toast";
import { useDragDrop } from "./lib/use-drag-drop";
import { useKeyboardShortcuts } from "./lib/use-keyboard-shortcuts";
import Bridge from "./routes/Bridge";
import Home from "./routes/Home";
import Packing from "./routes/Packing";
import Results from "./routes/Results";

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

/**
 * Subscribe to the `single-instance` event emitted by the backend when a
 * user tries to launch a second copy of the app (e.g. via "Open with"
 * or by dragging a folder onto the dock icon). The Rust side already
 * refocuses the existing window; the renderer's job here is to look at
 * the forwarded argv, find a path that resolves to a directory, and
 * patch it into the target so the user lands on the right repo.
 */
function useSingleInstance(): void {
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      const fn = await listen<string[]>("single-instance", async (event) => {
        // argv[0] is the binary path; skip it and look for the first
        // remaining argument that points to an existing directory.
        for (const arg of event.payload.slice(1)) {
          try {
            const info = await stat(arg);
            if (info.isDirectory) {
              // Don't yank options (or the view) out from under a pack
              // that's mid-run.
              if (useApp.getState().status === "running") return;
              useApp
                .getState()
                .patchOptions({ target: { kind: "folder", value: arg } });
              useApp.getState().setMoment("home");
              return;
            }
          } catch {
            // Unreadable / missing — keep scanning.
          }
        }
      });
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);
}

const TOAST_KIND_STYLES: Record<
  "info" | "success" | "error",
  { bg: string; text: string; ring: string; Icon: ComponentType<LucideProps> }
> = {
  info: {
    bg: "bg-zinc-800/95",
    text: "text-zinc-100",
    ring: "ring-zinc-700",
    Icon: AlertIcon,
  },
  success: {
    bg: "bg-zinc-900/90",
    text: "text-success",
    ring: "ring-success/40",
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
      // z-[300]: toasts must stay visible above the sheet overlay (z-[200]).
      aria-live="polite"
      aria-atomic="false"
      className="pointer-events-none fixed inset-x-0 bottom-0 z-[300] flex flex-col items-center gap-2 px-4 pb-4"
    >
      <AnimatePresence>
        {toasts.map((t) => {
          const style = TOAST_KIND_STYLES[t.kind] ?? TOAST_KIND_STYLES.info;
          const Icon = style.Icon;
          return (
            <m.button
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
            </m.button>
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

const MOMENTS = {
  home: Home,
  packing: Packing,
  results: Results,
  bridge: Bridge,
} as const;

function MomentView() {
  const moment = useApp((s) => s.moment);
  // Each moment is a fresh full-screen view — carry-over scroll position
  // from the previous moment is never meaningful.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `moment` is a deliberate trigger dep — the effect must re-run on every moment swap even though the body doesn't read it
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [moment]);
  const Active = MOMENTS[moment];
  return (
    <AnimatePresence mode="wait">
      <m.div
        key={moment}
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -12 }}
        transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
      >
        <Active />
      </m.div>
    </AnimatePresence>
  );
}

function Shell() {
  const activeSheet = useApp((s) => s.activeSheet);
  const setSheet = useApp((s) => s.setSheet);
  const setMoment = useApp((s) => s.setMoment);
  const patchOptions = useApp((s) => s.patchOptions);
  const status = useApp((s) => s.status);
  const { cancel } = usePackJobContext();

  const { isDragging, dropState } = useDragDrop({
    onDrop: (folderPath: string) => {
      if (useApp.getState().status === "running") return;
      patchOptions({ target: { kind: "folder", value: folderPath } });
      setMoment("home");
    },
  });

  useKeyboardShortcuts({
    "mod+k": () => {
      setSheet(null);
      if (useApp.getState().status !== "running") setMoment("home");
      // Store-driven focus handshake: TargetSection consumes (and clears)
      // the flag once mounted — a one-shot rAF would fire before the
      // moment-swap animation lets Home mount.
      useApp.getState().setPendingTargetFocus(true);
    },
    escape: () => {
      // Single Esc policy for the whole app (Packing registers no Esc
      // handler so two window listeners can't race on one keypress):
      // an open sheet closes first; otherwise a running pack cancels.
      if (useApp.getState().activeSheet) {
        setSheet(null);
        return;
      }
      if (useApp.getState().status === "running") void cancel();
    },
  });

  return (
    <div className="min-h-screen text-zinc-100">
      <DropOverlay
        visible={isDragging && status !== "running"}
        dropState={dropState}
      />
      <TitleBar />
      <MomentView />
      <Sheet
        open={activeSheet === "github"}
        title="GitHub"
        onClose={() => setSheet(null)}
      >
        <GithubConnector
          onSelectRepo={(htmlUrl) => {
            patchOptions({ target: { kind: "github", value: htmlUrl } });
            setSheet(null);
            if (useApp.getState().status !== "running") setMoment("home");
          }}
          onGoToSettings={() => setSheet("settings")}
        />
      </Sheet>
      <Sheet
        open={activeSheet === "settings"}
        title="Settings"
        onClose={() => setSheet(null)}
      >
        <Settings />
      </Sheet>
    </div>
  );
}

export default function App() {
  useSystemTheme();
  useSingleInstance();
  return (
    <ErrorBoundary FallbackComponent={ErrorFallback}>
      <LazyMotion features={domAnimation} strict>
        <MotionConfig
          reducedMotion="user"
          transition={{ duration: 0.18, ease: [0.2, 0, 0, 1] }}
        >
          <PackJobProvider>
            <Shell />
            <Toaster />
          </PackJobProvider>
        </MotionConfig>
      </LazyMotion>
    </ErrorBoundary>
  );
}
