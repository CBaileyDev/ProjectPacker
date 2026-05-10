import { dirname } from "@tauri-apps/api/path";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { stat } from "@tauri-apps/plugin-fs";
import { useEffect, useRef, useState } from "react";

export type DropState = "idle" | "valid" | "invalid";

interface UseDragDropOptions {
  /** Called with the resolved folder path when the user drops a folder. */
  onDrop: (folderPath: string) => void;
}

interface UseDragDropReturn {
  /** True while a drag is hovering the window (Tauri or HTML5 source). */
  isDragging: boolean;
  /** Visual state of the drop overlay: idle / valid (folder hovering) / invalid (file hovering). */
  dropState: DropState;
  /** Keyboard fallback that opens the OS folder picker via plugin-dialog. */
  openFileDialog: () => Promise<void>;
}

const INVALID_DROP_TIMEOUT_MS = 1_200;

/**
 * Resolve a path the user dropped to the directory we should operate on:
 *  - directory → return as-is
 *  - file → return its parent dir
 *  - inaccessible (permissions, deleted) → return the original path so the
 *    backend's error message is what the user sees, not a silent fallback.
 */
async function resolveFolderPath(path: string): Promise<string> {
  try {
    const info = await stat(path);
    if (info.isDirectory) return path;
  } catch {
    return path;
  }
  try {
    return await dirname(path);
  } catch {
    return path;
  }
}

/**
 * Probe `path` and return `true` only if it points to a directory. Used by
 * the Tauri drop handler so we can flash an "invalid" state when the user
 * drops a file instead of a folder. Errors collapse to `false` so an
 * inaccessible path is treated as not-a-directory.
 */
async function isDirectory(path: string): Promise<boolean> {
  try {
    const info = await stat(path);
    return info.isDirectory;
  } catch {
    return false;
  }
}

/**
 * Hook that wires Tauri 2's webview-window drag-drop event to React state,
 * with an HTML5 drag-listener fallback for browser-emulated environments
 * (e.g. `tauri dev` in a Linux container where the native drop event is
 * occasionally swallowed).
 *
 * Returns `{ isDragging, dropState, openFileDialog }`. `dropState` is
 * three-valued: `'idle'` while no drag is active, `'valid'` while a drag
 * hovers, and `'invalid'` for 1.2s after the user drops a non-folder.
 *
 * The HTML5 listeners use a `dragDepthRef` ref-counter (incremented on
 * `dragenter`, decremented on `dragleave`) to cancel out the dozens of
 * synthetic enter/leave pairs the browser fires as the cursor moves
 * between child elements. Without it, the overlay flickers on every
 * sub-element transition.
 */
export function useDragDrop({ onDrop }: UseDragDropOptions): UseDragDropReturn {
  const [isDragging, setIsDragging] = useState(false);
  const [dropState, setDropState] = useState<DropState>("idle");
  const onDropRef = useRef(onDrop);
  const isDraggingRef = useRef(false);
  const dragDepthRef = useRef(0);
  const invalidTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    onDropRef.current = onDrop;
  }, [onDrop]);

  function flashInvalid() {
    if (invalidTimerRef.current !== null) {
      clearTimeout(invalidTimerRef.current);
    }
    setDropState("invalid");
    invalidTimerRef.current = setTimeout(() => {
      setDropState("idle");
      invalidTimerRef.current = null;
    }, INVALID_DROP_TIMEOUT_MS);
  }

  function clearDrag() {
    isDraggingRef.current = false;
    dragDepthRef.current = 0;
    setIsDragging(false);
    // Don't reset dropState here if it's currently `'invalid'` — the
    // 1.2s timer owns clearing it.
    setDropState((prev) => (prev === "invalid" ? prev : "idle"));
  }

  function setDragActive() {
    if (!isDraggingRef.current) {
      isDraggingRef.current = true;
      setIsDragging(true);
    }
    setDropState((prev) => (prev === "invalid" ? prev : "valid"));
  }

  // ── Tauri webview drop wiring ──
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    (async () => {
      const win = getCurrentWebviewWindow();
      const fn = await win.onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === "enter" || t === "over") {
          // `over` fires ~60Hz while the cursor moves; only flip state on
          // the leading edge so we don't thrash through React's bail-out.
          setDragActive();
        } else if (t === "leave") {
          clearDrag();
        } else if (t === "drop") {
          isDraggingRef.current = false;
          setIsDragging(false);
          const paths = event.payload.paths ?? [];
          if (paths.length === 0) {
            setDropState((prev) => (prev === "invalid" ? prev : "idle"));
            return;
          }
          if (paths.length > 1) {
            console.warn(
              `[useDragDrop] ${paths.length} paths dropped; using first: ${paths[0]}`,
            );
          }
          (async () => {
            const ok = await isDirectory(paths[0]);
            if (!ok) {
              flashInvalid();
              return;
            }
            setDropState("idle");
            const folder = await resolveFolderPath(paths[0]);
            onDropRef.current(folder);
          })();
        }
      });

      // Race: cleanup may have run before this resolved. Honour it.
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

  // ── HTML5 fallback ──
  // Tauri's native dragdrop event sometimes fails to fire under WebKit on
  // older Linux GTK builds; the HTML5 listeners are the safety net. They
  // also fire in dev for non-Tauri renderers (e.g. unit tests in happy-dom).
  useEffect(() => {
    function onDragEnter(e: DragEvent) {
      e.preventDefault();
      dragDepthRef.current += 1;
      setDragActive();
    }
    function onDragOver(e: DragEvent) {
      // Required for the browser to fire a `drop` instead of cancelling.
      e.preventDefault();
    }
    function onDragLeave(e: DragEvent) {
      e.preventDefault();
      dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
      if (dragDepthRef.current === 0) {
        clearDrag();
      }
    }
    function onWindowDrop(e: DragEvent) {
      e.preventDefault();
      dragDepthRef.current = 0;
      isDraggingRef.current = false;
      setIsDragging(false);
      const items = e.dataTransfer?.items;
      // The Tauri-native handler will pick up the actual filesystem paths
      // via its own event; this branch only runs in the browser-emulated
      // path. We treat any non-file drop as invalid (DataTransfer's
      // `webkitGetAsEntry` is the only cross-browser way to detect
      // directory drops, and it's not exposed in happy-dom).
      let hasFile = false;
      let hasOnlyDirectories = true;
      if (items) {
        for (const item of Array.from(items)) {
          if (item.kind !== "file") continue;
          hasFile = true;
          // `webkitGetAsEntry` exists in Tauri's WebKit; gate the check
          // so happy-dom (which doesn't ship it) doesn't crash.
          // biome-ignore lint/suspicious/noExplicitAny: webkitGetAsEntry is non-standard
          const getEntry = (item as any).webkitGetAsEntry?.bind(item);
          if (typeof getEntry === "function") {
            const entry = getEntry();
            if (entry && !entry.isDirectory) hasOnlyDirectories = false;
          }
        }
      }
      if (hasFile && !hasOnlyDirectories) {
        flashInvalid();
      } else {
        setDropState((prev) => (prev === "invalid" ? prev : "idle"));
      }
    }

    window.addEventListener("dragenter", onDragEnter);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("dragleave", onDragLeave);
    window.addEventListener("drop", onWindowDrop);

    return () => {
      window.removeEventListener("dragenter", onDragEnter);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("dragleave", onDragLeave);
      window.removeEventListener("drop", onWindowDrop);
      if (invalidTimerRef.current !== null) {
        clearTimeout(invalidTimerRef.current);
        invalidTimerRef.current = null;
      }
    };
  }, []);

  async function openFileDialog() {
    const path = await openDialog({ directory: true });
    if (typeof path === "string") {
      onDropRef.current(path);
    }
  }

  return { isDragging, dropState, openFileDialog };
}
