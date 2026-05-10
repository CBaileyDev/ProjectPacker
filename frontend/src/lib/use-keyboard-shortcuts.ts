import { useEffect, useRef } from "react";

export type ShortcutMap = Record<string, () => void>;

/**
 * Detect whether `mod` should match `metaKey` (macOS Command) or
 * `ctrlKey` (everywhere else). We sniff `navigator.platform` rather
 * than UA so it stays correct under macOS Chromium-with-Linux UA spoof.
 */
function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  const platform = navigator.platform || "";
  return /Mac|iPhone|iPod|iPad/i.test(platform);
}

/**
 * Lower-cased canonical form of a keydown event for matching against
 * shortcut definitions like `'mod+c'`, `'esc'`, `'shift+/'`.
 *
 *  - `mod` → `metaKey` on macOS, `ctrlKey` elsewhere.
 *  - `cmd` and `meta` always map to `metaKey`.
 *  - `ctrl` always maps to `ctrlKey`.
 *  - Modifiers are emitted in alphabetical order so caller key order
 *    doesn't matter (`'mod+shift+a'` matches `'shift+mod+a'`).
 */
function normalizeShortcut(definition: string): string {
  const parts = definition
    .toLowerCase()
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  if (parts.length === 0) return "";

  const mac = isMacPlatform();
  const mods: string[] = [];
  let key = "";

  for (const part of parts) {
    switch (part) {
      case "mod":
        mods.push(mac ? "meta" : "ctrl");
        break;
      case "cmd":
      case "meta":
        mods.push("meta");
        break;
      case "ctrl":
      case "control":
        mods.push("ctrl");
        break;
      case "shift":
        mods.push("shift");
        break;
      case "alt":
      case "option":
        mods.push("alt");
        break;
      case "esc":
        key = "escape";
        break;
      case "space":
        key = " ";
        break;
      default:
        key = part;
    }
  }

  mods.sort();
  return mods.length > 0 ? `${mods.join("+")}+${key}` : key;
}

function eventToShortcut(e: KeyboardEvent): string {
  const mods: string[] = [];
  if (e.metaKey) mods.push("meta");
  if (e.ctrlKey) mods.push("ctrl");
  if (e.shiftKey) mods.push("shift");
  if (e.altKey) mods.push("alt");
  mods.sort();
  const key = e.key.toLowerCase();
  return mods.length > 0 ? `${mods.join("+")}+${key}` : key;
}

/**
 * Register a window-level keydown listener that fires the matching handler
 * from `map`. Keys may use the `mod` alias to map to Cmd on macOS / Ctrl
 * elsewhere (see `normalizeShortcut`).
 *
 * The `map` reference is stashed in a ref so callers don't have to memoize
 * it — re-passing a fresh object literal on every render is fine. The
 * listener itself only registers once per mount.
 *
 * Handlers fire only when the active element is NOT an editable text
 * surface (input/textarea/contenteditable) — typing `Cmd+C` inside an
 * input should still copy the selection, not fire your "copy result"
 * shortcut. Esc is the exception: it fires regardless so a modal can
 * always be dismissed.
 */
export function useKeyboardShortcuts(map: ShortcutMap): void {
  const mapRef = useRef(map);
  mapRef.current = map;

  useEffect(() => {
    function isEditable(el: EventTarget | null): boolean {
      if (!(el instanceof HTMLElement)) return false;
      if (el.isContentEditable) return true;
      const tag = el.tagName;
      return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
    }

    function handler(e: KeyboardEvent) {
      const observed = eventToShortcut(e);
      // Resolve the map's keys against the same canonical form each call
      // so the cost stays O(map size) per keypress — which is fine, since
      // the map is small (single-digit entries in practice).
      for (const [definition, fn] of Object.entries(mapRef.current)) {
        const target = normalizeShortcut(definition);
        if (target !== observed) continue;
        if (target !== "escape" && isEditable(e.target)) return;
        e.preventDefault();
        fn();
        return;
      }
    }

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
