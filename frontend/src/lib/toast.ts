import { create } from "zustand";

export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: string;
  message: string;
  kind: ToastKind;
  /** Auto-dismiss timeout in ms. 0 = sticky. */
  duration: number;
}

interface ToastOptions {
  kind?: ToastKind;
  /** ms before auto-dismiss; default 4000. Pass 0 to leave sticky. */
  duration?: number;
}

interface ToastState {
  toasts: Toast[];
  showToast: (message: string, opts?: ToastOptions) => string;
  dismissToast: (id: string) => void;
  clearToasts: () => void;
}

const DEFAULT_DURATION = 4_000;

let nextId = 0;
function genId(): string {
  // Local counter is enough — these IDs never leave the renderer and
  // don't need to be cryptographically unique.
  nextId += 1;
  return `toast-${Date.now()}-${nextId}`;
}

const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  showToast: (message, opts) => {
    const id = genId();
    const toast: Toast = {
      id,
      message,
      kind: opts?.kind ?? "info",
      duration: opts?.duration ?? DEFAULT_DURATION,
    };
    set((s) => ({ toasts: [...s.toasts, toast] }));
    if (toast.duration > 0) {
      setTimeout(() => {
        get().dismissToast(id);
      }, toast.duration);
    }
    return id;
  },
  dismissToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  clearToasts: () => set({ toasts: [] }),
}));

/**
 * Hook returning the current toast list and stable action handles.
 *
 * No `<Toaster />` is required for callers to invoke `showToast` — toasts
 * silently queue in the store, and a future renderer mount will pick them
 * up. This keeps `CopyButton` (and other call sites) unaware of whether a
 * toast UI is currently mounted.
 */
export function useToast() {
  const toasts = useToastStore((s) => s.toasts);
  const showToast = useToastStore((s) => s.showToast);
  const dismissToast = useToastStore((s) => s.dismissToast);
  return { toasts, showToast, dismissToast };
}
