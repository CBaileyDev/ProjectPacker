import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { type ReactNode, useEffect, useRef } from "react";
import { prefersReducedMotion } from "../../lib/motion";
import { XIcon } from "../pack/icons";

interface SheetProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/** Right-side overlay panel for GitHub / Settings. Scrim click and the ✕
 * button close it; Esc is handled globally in App. Focus moves into the
 * panel on open and back to the previously-focused element on close. */
export function Sheet({ open, title, onClose, children }: SheetProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (open) {
      restoreRef.current = document.activeElement as HTMLElement | null;
      panelRef.current?.focus();
    } else {
      const el = restoreRef.current;
      // The element may have unmounted while the sheet was open (moments
      // swap underneath sheets); restoring focus to a detached node is a
      // silent no-op, so only bother when it's still in the document.
      if (el && document.contains(el)) el.focus();
    }
  }, [open]);

  // z-[200]: above DropOverlay (z-50); the toast stack rides above the
  // sheet at z-[300] so toasts stay visible while a sheet is open.
  return (
    <AnimatePresence>
      {open && (
        <m.div
          className="fixed inset-0 z-[200] flex justify-end"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: prefersReducedMotion ? 0 : 0.15 }}
        >
          {/* Scrim click-to-close; Esc is handled globally in App, and the
           * panel's ✕ button is the keyboard-accessible close affordance. */}
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={onClose}
            aria-hidden="true"
          />
          <m.div
            ref={panelRef}
            role="dialog"
            aria-modal="true"
            aria-label={title}
            tabIndex={-1}
            className="relative flex h-full w-full max-w-xl flex-col overflow-y-auto border-l border-white/10 bg-background p-6 shadow-2xl outline-none"
            initial={
              prefersReducedMotion ? { opacity: 0 } : { x: 48, opacity: 0 }
            }
            animate={{ x: 0, opacity: 1 }}
            exit={prefersReducedMotion ? { opacity: 0 } : { x: 48, opacity: 0 }}
            transition={{
              duration: prefersReducedMotion ? 0 : 0.22,
              ease: [0.22, 1, 0.36, 1],
            }}
          >
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-lg font-bold tracking-tight">{title}</h2>
              <button
                type="button"
                onClick={onClose}
                aria-label={`Close ${title}`}
                className="rounded-lg p-2 text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
              >
                <XIcon size={16} />
              </button>
            </div>
            {children}
          </m.div>
        </m.div>
      )}
    </AnimatePresence>
  );
}
