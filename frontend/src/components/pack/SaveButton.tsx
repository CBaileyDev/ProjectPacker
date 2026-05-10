import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { memo, useCallback, useRef, useState } from "react";
import { commands } from "../../bindings";
import { springButton } from "../../lib/motion";
import { CheckIcon, LoaderIcon, SaveIcon, XIcon } from "./icons";

type SaveStatus =
  | { kind: "idle" }
  | { kind: "saving" }
  | { kind: "saved"; path: string }
  | { kind: "error"; message: string };

interface SaveButtonProps {
  label: string;
  suggestedFilename: string;
  text: string;
}

const SAVED_TIMEOUT_MS = 4_000;

/**
 * Save-to-file button. Calls `commands.savePackOutput` (which opens an OS
 * save dialog and writes the chosen path).
 *
 *  - `null` return from `savePackOutput` means the user cancelled the
 *    dialog — we silently revert to idle, no error toast.
 *  - `aria-busy` flips on while the dialog is open so AT users hear
 *    "busy" instead of "save".
 *  - The path/error tooltip is surfaced via `title=` so it shows on
 *    hover; we deliberately don't render it inline because the path
 *    can be 100+ chars and would line-wrap in tight layouts.
 */
function SaveButtonInner({ label, suggestedFilename, text }: SaveButtonProps) {
  const [status, setStatus] = useState<SaveStatus>({ kind: "idle" });
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const doSave = useCallback(async () => {
    if (resetTimerRef.current !== null) {
      clearTimeout(resetTimerRef.current);
      resetTimerRef.current = null;
    }
    setStatus({ kind: "saving" });
    try {
      const res = await commands.savePackOutput(suggestedFilename, text);
      if (res.status !== "ok") {
        setStatus({ kind: "error", message: res.error.message });
        return;
      }
      // null = user cancelled the OS save dialog — preserve previous
      // behavior: silently return to idle without flagging as error.
      if (res.data === null) {
        setStatus({ kind: "idle" });
        return;
      }
      setStatus({ kind: "saved", path: res.data });
      resetTimerRef.current = setTimeout(() => {
        setStatus({ kind: "idle" });
        resetTimerRef.current = null;
      }, SAVED_TIMEOUT_MS);
    } catch (e) {
      setStatus({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, [suggestedFilename, text]);

  const isSaving = status.kind === "saving";
  const isSaved = status.kind === "saved";
  const isError = status.kind === "error";

  return (
    <m.button
      type="button"
      onClick={doSave}
      disabled={isSaving}
      aria-busy={isSaving}
      aria-live="polite"
      aria-label={
        isSaved
          ? `Saved to ${status.path}`
          : isError
            ? `Save failed: ${status.message}`
            : label
      }
      title={isSaved ? status.path : isError ? status.message : undefined}
      whileTap={isSaving ? undefined : springButton}
      className={`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-sm font-medium transition-colors duration-200 focus-visible:outline-none disabled:cursor-wait ${
        isSaved
          ? "border-emerald-600/50 bg-emerald-500/10 text-emerald-400"
          : isError
            ? "border-red-600/50 bg-red-500/10 text-red-400"
            : "border-zinc-600/80 bg-zinc-800 text-zinc-200 hover:bg-zinc-700 hover:border-zinc-500"
      }`}
    >
      <AnimatePresence mode="wait" initial={false}>
        {isSaving ? (
          <m.span
            key="saving"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <m.span
              animate={{ rotate: 360 }}
              transition={{ duration: 1.2, repeat: Infinity, ease: "linear" }}
            >
              <LoaderIcon size={15} />
            </m.span>
            Saving…
          </m.span>
        ) : isSaved ? (
          <m.span
            key="saved"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <CheckIcon size={15} />
            Saved
          </m.span>
        ) : isError ? (
          <m.span
            key="error"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <XIcon size={15} />
            Failed
          </m.span>
        ) : (
          <m.span
            key="idle"
            className="flex items-center gap-2"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ type: "spring", stiffness: 500, damping: 25 }}
          >
            <SaveIcon size={15} />
            {label}
          </m.span>
        )}
      </AnimatePresence>
    </m.button>
  );
}

export const SaveButton = memo(SaveButtonInner);
