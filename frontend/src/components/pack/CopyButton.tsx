import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { AnimatePresence, motion } from "framer-motion";
// NOTE: useState MUST be imported here. The previous version of this file
// shipped without it (memo/useCallback/useRef were imported but useState
// was forgotten); the swarm-coordinator deliverable flagged it as "BUG:
// Missing useState import". Don't drop it again.
import { memo, useCallback, useRef, useState } from "react";
import { springButton } from "../../lib/motion";
import { useToast } from "../../lib/toast";
import { useKeyboardShortcuts } from "../../lib/use-keyboard-shortcuts";
import { CheckIcon, CopyIcon, XIcon } from "./icons";

type CopyStatus =
  | { kind: "idle" }
  | { kind: "copied" }
  | { kind: "error"; message: string };

interface CopyButtonProps {
  label: string;
  text: string;
}

const COPIED_TIMEOUT_MS = 2_000;
const ERROR_TIMEOUT_MS = 4_000;

/**
 * Copy-to-clipboard button with toast + AnimatePresence state transitions.
 *
 *  - Wires `mod+c` (Cmd on macOS, Ctrl elsewhere) to fire the same handler
 *    while this button is mounted. The shortcut hook already gates on
 *    editable elements so typing Cmd+C in a textarea still copies the
 *    selection rather than the pack output.
 *  - States idle / copied / error each carry their own AnimatePresence
 *    child so the icon swap is animated, not abrupt.
 *  - Toasts surface success/failure outside the button — the button's own
 *    state machine resets after 2s/4s, so without a toast the user might
 *    miss a fast click.
 */
function CopyButtonInner({ label, text }: CopyButtonProps) {
  const [status, setStatus] = useState<CopyStatus>({ kind: "idle" });
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { showToast } = useToast();

  const doCopy = useCallback(async () => {
    if (resetTimerRef.current !== null) {
      clearTimeout(resetTimerRef.current);
      resetTimerRef.current = null;
    }
    try {
      await writeText(text);
      setStatus({ kind: "copied" });
      showToast("Copied to clipboard", { kind: "success" });
      resetTimerRef.current = setTimeout(() => {
        setStatus({ kind: "idle" });
        resetTimerRef.current = null;
      }, COPIED_TIMEOUT_MS);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setStatus({ kind: "error", message });
      showToast(`Copy failed: ${message}`, { kind: "error" });
      resetTimerRef.current = setTimeout(() => {
        setStatus({ kind: "idle" });
        resetTimerRef.current = null;
      }, ERROR_TIMEOUT_MS);
    }
  }, [text, showToast]);

  // Pass the live handler through the shortcut map; the hook stashes it
  // in a ref internally so we can re-pass a fresh closure each render.
  useKeyboardShortcuts({ "mod+c": doCopy });

  const isCopied = status.kind === "copied";
  const isError = status.kind === "error";

  return (
    <motion.button
      type="button"
      onClick={doCopy}
      title={isError ? status.message : undefined}
      aria-label={
        isCopied ? "Copied to clipboard" : isError ? "Copy failed" : label
      }
      aria-live="polite"
      whileTap={springButton}
      className={`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-sm font-medium transition-colors duration-200 focus-visible:outline-none ${
        isCopied
          ? "border-emerald-600/50 bg-emerald-500/10 text-emerald-400"
          : isError
            ? "border-red-600/50 bg-red-500/10 text-red-400"
            : "border-zinc-600/80 bg-zinc-800 text-zinc-200 hover:bg-zinc-700 hover:border-zinc-500"
      }`}
    >
      <AnimatePresence mode="wait" initial={false}>
        {isCopied ? (
          <motion.span
            key="copied"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <CheckIcon size={15} />
            Copied!
          </motion.span>
        ) : isError ? (
          <motion.span
            key="error"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <XIcon size={15} />
            Failed
          </motion.span>
        ) : (
          <motion.span
            key="idle"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <CopyIcon size={15} />
            {label}
          </motion.span>
        )}
      </AnimatePresence>
    </motion.button>
  );
}

export const CopyButton = memo(CopyButtonInner);
