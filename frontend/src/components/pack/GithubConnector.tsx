import { motion } from "framer-motion";
import { useEffect, useMemo, useState } from "react";
import { commands, type GithubRepo } from "../../bindings";
import { fadeUp, springButton } from "../../lib/motion";
import { useGithubToken } from "../../lib/use-github-token";
import { SkeletonRow } from "./Skeleton";
import {
  AlertIcon,
  GithubIcon,
  LockIcon,
  RefreshIcon,
  SearchIcon,
  SettingsIcon,
  StarIcon,
} from "./icons";

interface GithubConnectorProps {
  onSelectRepo: (htmlUrl: string) => void;
  onGoToSettings: () => void;
}

export function GithubConnector({
  onSelectRepo,
  onGoToSettings,
}: GithubConnectorProps) {
  const { hasToken, ready } = useGithubToken();
  const [repos, setRepos] = useState<GithubRepo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  async function loadRepos() {
    setLoading(true);
    setError(null);
    const res = await commands.githubListRepos();
    setLoading(false);
    if (res.status === "ok") {
      setRepos(res.data);
      return;
    }
    // Map the typed Rust error codes to friendly UI text.
    const err = res.error;
    setError(
      err.code === "github_unauthorized"
        ? "GitHub rejected the token. Check Settings — it may have expired or be missing the `repo` scope."
        : err.code === "github_rate_limit"
          ? "GitHub API rate limit reached. Try again in a few minutes."
          : err.code === "github_no_token"
            ? "No token stored. Add one in Settings to browse your repos."
            : err.code === "github_forbidden"
              ? `GitHub forbidden the request: ${err.message}`
              : err.message,
    );
  }

  // Auto-load when ready + connected. Re-runs after a Settings save flips
  // hasToken from false to true.
  useEffect(() => {
    if (!ready) return;
    if (!hasToken) {
      setRepos([]);
      return;
    }
    void loadRepos();
  }, [hasToken, ready]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return repos;
    return repos.filter(
      (r) =>
        r.fullName.toLowerCase().includes(q) ||
        (r.description?.toLowerCase().includes(q) ?? false) ||
        (r.language?.toLowerCase().includes(q) ?? false),
    );
  }, [repos, filter]);

  // ── No PAT yet ──────────────────────────────────────────────────────────
  if (ready && !hasToken) {
    return (
      <motion.div
        className="flex min-h-[360px] flex-col items-center justify-center rounded-2xl border border-dashed border-zinc-700/70 bg-zinc-900/25 px-6 text-center"
        variants={fadeUp}
        initial="hidden"
        animate="visible"
      >
        <GithubIcon size={32} className="text-zinc-500" />
        <h3 className="mt-4 text-base font-semibold text-zinc-200">
          Connect to GitHub to browse repos
        </h3>
        <p className="mt-2 max-w-md text-sm leading-relaxed text-zinc-500">
          Add a Personal Access Token in Settings — it's stored in a
          user-only file in your app-data folder and never reaches the
          renderer. Once connected, your repos appear here and a single
          click loads one into the Packer tab.
        </p>
        <motion.button
          type="button"
          className="mt-5 flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2.5 text-sm font-semibold text-white shadow-lg shadow-emerald-900/30 hover:bg-emerald-500"
          onClick={onGoToSettings}
          whileTap={springButton}
        >
          <SettingsIcon size={15} />
          Open Settings
        </motion.button>
      </motion.div>
    );
  }

  return (
    <motion.div
      className="space-y-4"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 min-w-[220px]">
          <SearchIcon
            size={14}
            className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500"
          />
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter by name, description, or language…"
            aria-label="Filter repositories"
            className="w-full rounded-lg border border-zinc-700 bg-zinc-800/60 pl-9 pr-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:border-emerald-500/50 focus:outline-none"
          />
        </div>
        <motion.button
          type="button"
          onClick={() => void loadRepos()}
          disabled={loading || !hasToken}
          className="flex items-center gap-1.5 rounded-lg border border-zinc-600 bg-zinc-800 px-3.5 py-2.5 text-sm text-zinc-200 hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
          whileTap={springButton}
          aria-label="Refresh repository list"
        >
          <motion.span
            aria-hidden="true"
            animate={loading ? { rotate: 360 } : { rotate: 0 }}
            transition={
              loading
                ? { duration: 1, repeat: Infinity, ease: "linear" }
                : { duration: 0.3 }
            }
          >
            <RefreshIcon size={14} />
          </motion.span>
          {loading ? "Loading…" : "Refresh"}
        </motion.button>
      </div>

      {error && (
        <motion.div
          role="alert"
          className="flex items-start gap-3 rounded-xl border border-red-600/40 bg-red-950/40 px-4 py-3 text-sm text-red-300"
          initial={{ opacity: 0, y: -4 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <AlertIcon size={16} className="mt-0.5 shrink-0 text-red-400" />
          <div className="flex-1 break-words">{error}</div>
        </motion.div>
      )}

      {loading && repos.length === 0 && (
        <div className="overflow-hidden rounded-xl border border-zinc-700/80">
          <div className="divide-y divide-zinc-800/60 px-4">
            {Array.from({ length: 6 }).map((_, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: static placeholder
              <SkeletonRow key={i} />
            ))}
          </div>
        </div>
      )}

      {!loading && filtered.length === 0 && repos.length > 0 && (
        <div className="rounded-xl border border-zinc-700/80 bg-zinc-800/40 p-5 text-sm text-zinc-400">
          No repos match <span className="font-mono">"{filter}"</span>.
        </div>
      )}

      {filtered.length > 0 && (
        <div className="overflow-hidden rounded-xl border border-zinc-700/80">
          <div className="divide-y divide-zinc-800/60">
            {filtered.map((repo) => (
              <RepoRow
                key={repo.id}
                repo={repo}
                onClick={() => onSelectRepo(repo.htmlUrl)}
              />
            ))}
          </div>
        </div>
      )}

      {!loading && repos.length > 0 && (
        <p className="text-xs text-zinc-600">
          {filtered.length} of {repos.length} repos shown · sorted by latest
          push
        </p>
      )}
    </motion.div>
  );
}

function RepoRow({ repo, onClick }: { repo: GithubRepo; onClick: () => void }) {
  return (
    <motion.button
      type="button"
      onClick={onClick}
      className="flex w-full items-start gap-3 px-4 py-3 text-left transition-colors hover:bg-zinc-800/50"
      whileTap={{ scale: 0.995 }}
    >
      <img
        src={repo.owner.avatarUrl}
        alt=""
        width={32}
        height={32}
        referrerPolicy="no-referrer"
        className="mt-0.5 h-8 w-8 shrink-0 rounded-md bg-zinc-800"
      />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-semibold text-zinc-100">
            {repo.fullName}
          </span>
          {repo.private && (
            <span
              className="flex shrink-0 items-center gap-1 rounded bg-zinc-700/60 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-zinc-300"
              title="Private repository"
            >
              <LockIcon size={10} />
              Private
            </span>
          )}
          {repo.archived && (
            <span className="shrink-0 rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-amber-300">
              Archived
            </span>
          )}
        </div>
        {repo.description && (
          <p className="mt-0.5 truncate text-xs text-zinc-500">
            {repo.description}
          </p>
        )}
        <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-zinc-500">
          {repo.language && (
            <span className="text-zinc-400">{repo.language}</span>
          )}
          {repo.stargazersCount > 0 && (
            <span className="flex items-center gap-1">
              <StarIcon size={10} />
              {repo.stargazersCount.toLocaleString()}
            </span>
          )}
          <span>
            updated {new Date(repo.pushedAt).toLocaleDateString()}
          </span>
        </div>
      </div>
    </motion.button>
  );
}
