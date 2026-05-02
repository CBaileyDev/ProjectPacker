import { getCurrentWebview } from "@tauri-apps/api/webview";
import { stat } from "@tauri-apps/plugin-fs";
import { useEffect, useState } from "react";

/**
 * Resolve a dropped path into a folder path.
 *
 * - If the path is a directory, returns it unchanged.
 * - If the path is a file, returns its parent directory.
 * - If `stat()` fails (race / permission), returns the path as-is and lets
 *   the orchestrator surface any error.
 */
async function resolveFolderPath(path: string): Promise<string> {
  try {
    const info = await stat(path);
    if (info.isDirectory) return path;
  } catch {
    // stat failed — fall through to path-as-is.
    return path;
  }
  // It's a file. Use the parent directory. Cross-platform split on both
  // separators so Windows paths (`C:\foo\bar.txt`) and POSIX paths
  // (`/foo/bar.txt`) both work.
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (idx <= 0) return path;
  return path.slice(0, idx);
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
 */
export function useDragDrop({ onDrop }: UseDragDropOptions): { isDragging: boolean } {
  const [isDragging, setIsDragging] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const webview = getCurrentWebview();
      unlisten = await webview.onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === "enter" || t === "over") {
          setIsDragging(true);
        } else if (t === "leave") {
          setIsDragging(false);
        } else if (t === "drop") {
          setIsDragging(false);
          const paths = event.payload.paths ?? [];
          if (paths.length === 0) return;
          if (paths.length > 1) {
            console.warn(
              `[useDragDrop] ${paths.length} paths dropped; using first: ${paths[0]}`,
            );
          }
          resolveFolderPath(paths[0]).then(onDrop);
        }
      });
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [onDrop]);

  return { isDragging };
}
