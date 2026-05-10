import * as m from "framer-motion/m";
import { useState } from "react";
import { commands, type GithubUser } from "../../bindings";
import { fadeUp, springButton } from "../../lib/motion";
import { useGithubToken } from "../../lib/use-github-token";
import {
  AlertIcon,
  CheckIcon,
  EyeIcon,
  EyeOffIcon,
  GithubIcon,
  LockIcon,
} from "./icons";

export function Settings() {
  const { hasToken, ready, setToken, clearToken } = useGithubToken();
  const [draft, setDraft] = useState<string>("");
  const [reveal, setReveal] = useState(false);
  const [saving, setSaving] = useState(false);
  // Save errors are kept distinct from connection-test errors. A
  // format/keychain rejection happens before any network call and the
  // user needs to see it right by the Save button.
  const [saveError, setSaveError] = useState<string | null>(null);
  // Brief "Saved" confirmation after a successful keychain write.
  // Independent from the connection test below — saving and verifying
  // against GitHub are two separate operations.
  const [justSaved, setJustSaved] = useState(false);
  const [testStatus, setTestStatus] = useState<
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; user: GithubUser }
    | { kind: "error"; message: string }
  >({ kind: "idle" });

  const isDirty = draft.length > 0;

  async function handleSave() {
    if (!isDirty) return;
    setSaving(true);
    setSaveError(null);
    setJustSaved(false);
    try {
      await setToken(draft.trim());
      // Scrub the draft buffer the moment the keychain accepts it. Without
      // this the unsaved-PAT string would linger in React state until the
      // tab unmounts — every dev-tools snapshot in between leaks it.
      setDraft("");
      setReveal(false);
      setJustSaved(true);
      // Auto-clear the success after a moment so the next save attempt
      // shows "Saving…" cleanly.
      setTimeout(() => setJustSaved(false), 3000);
      // Deliberately NOT auto-running runTestConnection here. A
      // misbehaving network round-trip used to make the save *appear*
      // to fail when the keychain write had actually succeeded — keep
      // the two operations independent. The user can click "Test
      // connection" below when they want to verify against GitHub.
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function runTestConnection() {
    setTestStatus({ kind: "testing" });
    const res = await commands.githubGetUser();
    if (res.status === "ok") {
      setTestStatus({ kind: "ok", user: res.data });
    } else {
      // The Rust error code already maps 401 vs 403 vs rate-limit etc.
      const err = res.error;
      const msg =
        err.code === "github_unauthorized"
          ? "Token rejected by GitHub. Check it has `repo` scope and hasn't expired."
          : err.code === "github_rate_limit"
            ? "GitHub API rate limit reached. Try again in a few minutes."
            : err.code === "github_no_token"
              ? "No token stored yet — save one above first."
              : err.message;
      setTestStatus({ kind: "error", message: msg });
    }
  }

  async function handleClear() {
    // Belt-and-suspenders: clear the input draft + reveal flag too so
    // nothing leftover from a paste-and-disconnect sequence sticks.
    setDraft("");
    setReveal(false);
    try {
      await clearToken();
      setTestStatus({ kind: "idle" });
    } catch (e) {
      setTestStatus({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }

  return (
    <m.div
      className="space-y-6"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      {/* GitHub section */}
      <section className="space-y-3 rounded-2xl border border-zinc-800 bg-zinc-900/40 p-5">
        <div className="flex items-center gap-2">
          <GithubIcon size={16} className="text-zinc-300" />
          <h3 className="text-sm font-semibold text-zinc-100">GitHub</h3>
          {ready && hasToken && (
            <span className="ml-auto flex items-center gap-1 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-emerald-300">
              <CheckIcon size={10} />
              Connected
            </span>
          )}
        </div>
        <p className="text-xs leading-relaxed text-zinc-500">
          A Personal Access Token lets ProjectPacker list your repositories on
          the GitHub tab and clone private repos. Create one at{" "}
          <span className="font-mono text-zinc-400">
            github.com/settings/tokens
          </span>{" "}
          with the <span className="font-mono">repo</span> scope (or{" "}
          <span className="font-mono">public_repo</span> if you only need public
          repos).
        </p>

        <div className="mt-1 flex items-center gap-2 rounded-lg border border-emerald-700/30 bg-emerald-950/20 px-3 py-2 text-[11px] text-emerald-300/85">
          <LockIcon size={12} className="shrink-0" />
          Stored in a user-only file under your app-data folder. Never visible
          to the renderer process.
        </div>

        <div className="mt-3 space-y-2">
          <label
            htmlFor="github-token-input"
            className="block text-xs font-semibold uppercase tracking-wider text-zinc-500"
          >
            {hasToken ? "Replace token" : "Personal Access Token"}
          </label>
          <div className="flex gap-2">
            <div className="relative flex-1">
              <input
                id="github-token-input"
                type={reveal ? "text" : "password"}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                placeholder={
                  hasToken
                    ? "Paste a new token to replace the stored one"
                    : "ghp_… or github_pat_…"
                }
                autoComplete="off"
                spellCheck={false}
                className="w-full rounded-lg border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 pr-10 font-mono text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:border-emerald-500/50 focus:outline-none"
                disabled={!ready || saving}
              />
              <button
                type="button"
                onClick={() => setReveal((v) => !v)}
                aria-label={reveal ? "Hide token" : "Show token"}
                className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1.5 text-zinc-500 hover:bg-zinc-700/50 hover:text-zinc-300"
              >
                {reveal ? <EyeOffIcon size={14} /> : <EyeIcon size={14} />}
              </button>
            </div>
            <m.button
              type="button"
              onClick={handleSave}
              disabled={!isDirty || saving}
              className={`rounded-lg px-4 py-2.5 text-sm font-semibold transition-colors ${
                isDirty && !saving
                  ? "bg-emerald-600 text-white hover:bg-emerald-500"
                  : "cursor-not-allowed bg-zinc-800 text-zinc-500"
              }`}
              whileTap={isDirty && !saving ? springButton : undefined}
            >
              {saving ? "Saving…" : "Save"}
            </m.button>
          </div>
          <div className="flex flex-wrap gap-2 pt-1">
            <m.button
              type="button"
              onClick={runTestConnection}
              disabled={!hasToken || testStatus.kind === "testing"}
              className="rounded-lg border border-zinc-600 bg-zinc-800 px-3.5 py-2 text-xs text-zinc-200 hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
              whileTap={springButton}
            >
              {testStatus.kind === "testing" ? "Testing…" : "Test connection"}
            </m.button>
            {hasToken && (
              <m.button
                type="button"
                onClick={handleClear}
                className="rounded-lg border border-red-700/40 bg-red-900/20 px-3.5 py-2 text-xs text-red-300 hover:bg-red-900/40"
                whileTap={springButton}
              >
                Disconnect
              </m.button>
            )}
          </div>

          {/* Save success banner — the keychain accepted the token. The
              connection test is now opt-in (separate button). */}
          {justSaved && !saveError && (
            <m.div
              role="status"
              className="mt-3 flex items-center gap-2 rounded-lg border border-emerald-700/40 bg-emerald-950/30 px-3 py-2.5 text-sm text-emerald-300"
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
            >
              <CheckIcon size={14} className="shrink-0" />
              <div className="flex-1 text-xs">
                Saved to keychain. Click "Test connection" to verify it works
                against GitHub.
              </div>
            </m.div>
          )}

          {/* Save error banner — distinct from the connection-test status
              below, since a format/keychain rejection happens before any
              network call and the user needs to see it immediately. */}
          {saveError && (
            <m.div
              role="alert"
              className="mt-3 flex items-start gap-3 rounded-lg border border-red-600/40 bg-red-950/30 px-3 py-2.5 text-sm text-red-300"
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
            >
              <AlertIcon size={14} className="mt-0.5 shrink-0 text-red-400" />
              <div className="flex-1 break-words text-xs">{saveError}</div>
              <button
                type="button"
                onClick={() => setSaveError(null)}
                aria-label="Dismiss save error"
                className="-mr-1 -mt-1 shrink-0 rounded p-1 text-red-300/80 hover:bg-red-900/40 hover:text-red-200 transition-colors"
              >
                <svg
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </m.div>
          )}
        </div>

        {/* Connection status */}
        {testStatus.kind === "ok" && (
          <m.div
            className="mt-3 flex items-center gap-3 rounded-lg border border-emerald-700/40 bg-emerald-950/30 px-3 py-2.5"
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <img
              src={testStatus.user.avatarUrl}
              alt=""
              width={28}
              height={28}
              referrerPolicy="no-referrer"
              className="h-7 w-7 rounded-full bg-zinc-800"
            />
            <div className="flex-1 text-sm">
              <div className="flex items-center gap-1.5 font-semibold text-emerald-300">
                <CheckIcon size={13} />
                Connected as {testStatus.user.login}
              </div>
              <div className="text-xs text-zinc-500">
                {testStatus.user.publicRepos} public repos
                {testStatus.user.name ? ` · ${testStatus.user.name}` : ""}
              </div>
            </div>
          </m.div>
        )}
        {testStatus.kind === "error" && (
          <m.div
            role="alert"
            className="mt-3 flex items-start gap-3 rounded-lg border border-red-600/40 bg-red-950/30 px-3 py-2.5 text-sm text-red-300"
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <AlertIcon size={14} className="mt-0.5 shrink-0 text-red-400" />
            <div className="flex-1 break-words text-xs">
              {testStatus.message}
            </div>
          </m.div>
        )}
      </section>

      {/* Build info */}
      <section className="space-y-2 rounded-2xl border border-zinc-800 bg-zinc-900/40 p-5">
        <h3 className="text-sm font-semibold text-zinc-100">About</h3>
        <p className="text-xs leading-relaxed text-zinc-500">
          ProjectPacker bundles a folder or repo into a single AI-ready file.
          Settings persist at{" "}
          <span className="font-mono">
            ~/Library/Application Support/dev.cbailey.projectpacker/
          </span>
          . The GitHub PAT is stored alongside as{" "}
          <span className="font-mono">github-token</span> with{" "}
          <span className="font-mono">0600</span> permissions.
        </p>
      </section>
    </m.div>
  );
}
