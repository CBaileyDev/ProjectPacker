import { PROTOCOL_VERSION } from "../../lib/protocol";
import { useApp, usePackProgress } from "../../lib/store";
import { useGithubToken } from "../../lib/use-github-token";
import { isMacPlatform } from "../../lib/use-keyboard-shortcuts";
import { GithubIcon, PackageIcon, SettingsIcon } from "../pack/icons";

/** Persistent top strip: wordmark (click → Home), ⌘K hint, GitHub status,
 * settings gear. The only chrome that survives across moments. */
export function TitleBar() {
  const setMoment = useApp((s) => s.setMoment);
  const setSheet = useApp((s) => s.setSheet);
  const { status } = usePackProgress();
  const { hasToken, ready } = useGithubToken();

  return (
    <header className="flex items-center gap-3 px-5 py-3">
      <button
        type="button"
        onClick={() => {
          if (status !== "running") setMoment("home");
        }}
        className="flex items-center gap-2.5 rounded-lg px-1.5 py-1 transition-colors hover:bg-white/5"
        aria-label="Go to Home"
      >
        <span className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary/15 ring-1 ring-primary/25">
          <PackageIcon size={15} className="text-primary" />
        </span>
        <span className="text-sm font-bold tracking-tight">ProjectPacker</span>
      </button>

      <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-primary/80">
        {PROTOCOL_VERSION}
      </span>

      <div className="ml-auto flex items-center gap-1.5">
        <kbd className="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 font-sans text-[10px] text-zinc-500">
          {isMacPlatform() ? "⌘K" : "Ctrl K"}
        </kbd>
        <button
          type="button"
          onClick={() => setSheet("github")}
          aria-label="GitHub"
          title={ready && hasToken ? "GitHub — connected" : "GitHub"}
          className="relative rounded-lg p-2 text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
        >
          <GithubIcon size={16} />
          {ready && hasToken && (
            <span
              aria-hidden="true"
              className="absolute right-1 top-1 h-1.5 w-1.5 rounded-full bg-primary"
            />
          )}
        </button>
        <button
          type="button"
          onClick={() => setSheet("settings")}
          aria-label="Settings"
          className="rounded-lg p-2 text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
        >
          <SettingsIcon size={16} />
        </button>
      </div>
    </header>
  );
}
