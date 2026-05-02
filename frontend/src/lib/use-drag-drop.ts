import { dirname } from "@tauri-apps/api/path";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { stat } from "@tauri-apps/plugin-fs";
import { useEffect, useRef, useState } from "react";

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

interface UseDragDropOptions {
  /** Called with the resolved folder path when the user drops a folder/file. */
  onDrop: (folderPath: string) => void;
}

/**
 * Hook that wires Tauri 2's webview-level drag-drop event to React state.
 *
 * Returns `{ isDragging }` so consumers can show a drop overlay while a
 * drag is hovering the window. The `onDrop` callback fires with the first
 * dropped path resolved to a folder (parent dir for files).
 *
 * The listener registers exactly once per mount: `onDrop` is read through a
 * ref so callers don't have to memoize it. Tauri's `onDragDropEvent`
 * registers four IPC listeners (enter/over/drop/leave) per call, and a
 * sustained drag-hover would otherwise tear down + re-subscribe them on
 * every parent render — and leak listeners if a render arrives before the
 * async `unlisten` resolves.
 */
export function useDragDrop({ onDrop }: UseDragDropOptions): { isDragging: boolean } {
  const [isDragging, setIsDragging] = useState(false);
  const onDropRef = useRef(onDrop);
  const isDraggingRef = useRef(false);

  useEffect(() => {
    onDropRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    (async () => {
      const webview = getCurrentWebview();
      const fn = await webview.onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === "enter" || t === "over") {
          // `over` fires ~60Hz while the cursor moves; only flip state on
          // the leading edge so we don't thrash through React's bail-out.
          if (!isDraggingRef.current) {
            isDraggingRef.current = true;
            setIsDragging(true);
          }
        } else if (t === "leave") {
          isDraggingRef.current = false;
          setIsDragging(false);
        } else if (t === "drop") {
          isDraggingRef.current = false;
          setIsDragging(false);
          const paths = event.payload.paths ?? [];
          if (paths.length === 0) return;
          if (paths.length > 1) {
            console.warn(
              `[useDragDrop] ${paths.length} paths dropped; using first: ${paths[0]}`,
            );
          }
          resolveFolderPath(paths[0]).then((folder) => onDropRef.current(folder));
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

  return { isDragging };
}
