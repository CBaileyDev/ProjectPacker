import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { useEffect } from "react";
import { prefersReducedMotion, springButton } from "../../lib/motion";
import { isValidTargetValue } from "../../lib/pack-meta";
import { useApp } from "../../lib/store";
import { FolderIcon, GithubIcon } from "../pack/icons";
import { SectionTitle } from "./SectionTitle";

// ─────────────────────────────────────────────────────────────────────────
// TargetSection — the folder / GitHub-URL target picker. Uses field-level
// store selectors so a keystroke in these inputs re-renders only this
// component (subscribed to `options.target`), not the whole Home moment.
// ─────────────────────────────────────────────────────────────────────────

export function TargetSection({
  onBrowseGithub,
}: {
  onBrowseGithub: () => void;
}) {
  const target = useApp((s) => s.options.target);
  const patchOptions = useApp((s) => s.patchOptions);
  const pendingTargetFocus = useApp((s) => s.pendingTargetFocus);
  const setPendingTargetFocus = useApp((s) => s.setPendingTargetFocus);

  // mod+k handshake: the global handler sets the flag (possibly before
  // Home is mounted — AnimatePresence mode="wait" delays the swap); the
  // effect runs on mount and on flag changes, so the focus lands as soon
  // as this section exists.
  useEffect(() => {
    if (pendingTargetFocus) {
      document.getElementById("target-input")?.focus();
      setPendingTargetFocus(false);
    }
  }, [pendingTargetFocus, setPendingTargetFocus]);

  const targetMode = target.kind;
  const targetVal = target.value;
  const isValidTarget = isValidTargetValue(targetMode, targetVal);

  function setTargetMode(mode: "folder" | "github") {
    patchOptions({ target: { kind: mode, value: "" } });
  }

  async function pickFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      patchOptions({ target: { kind: "folder", value: path } });
    }
  }

  return (
    <section className="space-y-3">
      <SectionTitle>Target</SectionTitle>

      <div className="flex w-fit gap-1.5 rounded-lg border border-zinc-700/50 bg-zinc-800/60 p-1">
        <button
          type="button"
          onClick={() => setTargetMode("folder")}
          aria-pressed={targetMode === "folder"}
          className={`flex items-center gap-1.5 rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200 ${
            targetMode === "folder"
              ? "bg-primary text-primary-foreground shadow-lg shadow-black/30"
              : "text-zinc-400 hover:text-zinc-200"
          }`}
        >
          <FolderIcon size={14} />
          Folder
        </button>
        <button
          type="button"
          onClick={() => setTargetMode("github")}
          aria-pressed={targetMode === "github"}
          className={`flex items-center gap-1.5 rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200 ${
            targetMode === "github"
              ? "bg-primary text-primary-foreground shadow-lg shadow-black/30"
              : "text-zinc-400 hover:text-zinc-200"
          }`}
        >
          <GithubIcon size={14} />
          GitHub URL
        </button>
      </div>

      <AnimatePresence mode="wait">
        {targetMode === "folder" ? (
          <m.div
            key="folder"
            className="flex gap-2"
            initial={prefersReducedMotion ? false : { opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
            transition={{
              duration: prefersReducedMotion ? 0 : 0.2,
            }}
          >
            <input
              id="target-input"
              className="flex-1 rounded-lg border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:border-primary/50 focus:outline-none"
              value={targetVal}
              placeholder="/path/to/project"
              aria-label="Folder path"
              onChange={(e) =>
                patchOptions({
                  target: {
                    kind: "folder",
                    value: e.target.value,
                  },
                })
              }
            />
            <m.button
              type="button"
              className="rounded-lg border border-zinc-600 bg-zinc-700 px-4 py-2.5 text-sm text-zinc-200 hover:bg-zinc-600 transition-colors"
              onClick={pickFolder}
              whileTap={springButton}
            >
              Browse…
            </m.button>
          </m.div>
        ) : (
          <m.div
            key="github"
            initial={prefersReducedMotion ? false : { opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={prefersReducedMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
            transition={{
              duration: prefersReducedMotion ? 0 : 0.2,
            }}
          >
            <div className="flex gap-2">
              <input
                id="target-input"
                className={`flex-1 rounded-lg border bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:outline-none ${
                  targetVal && !isValidTarget
                    ? "border-red-600 focus:border-red-500"
                    : "border-zinc-700 focus:border-primary/50"
                }`}
                value={targetVal}
                placeholder="https://github.com/owner/repo"
                aria-label="GitHub repository URL"
                aria-invalid={Boolean(targetVal) && !isValidTarget}
                onChange={(e) =>
                  patchOptions({
                    target: {
                      kind: "github",
                      value: e.target.value,
                    },
                  })
                }
              />
              <m.button
                type="button"
                className="rounded-lg border border-zinc-600 bg-zinc-700 px-4 py-2.5 text-sm text-zinc-200 hover:bg-zinc-600 transition-colors"
                onClick={onBrowseGithub}
                whileTap={springButton}
                title="Pick a repo from the GitHub tab"
              >
                Browse…
              </m.button>
            </div>
            {targetVal && !isValidTarget && (
              <m.div
                className="mt-1.5 text-xs text-red-400"
                role="alert"
                initial={{ opacity: 0, y: -4 }}
                animate={{ opacity: 1, y: 0 }}
              >
                Enter a valid GitHub repo URL, such as
                https://github.com/owner/repo
              </m.div>
            )}
          </m.div>
        )}
      </AnimatePresence>
    </section>
  );
}
