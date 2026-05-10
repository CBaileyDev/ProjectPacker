import { LazyStore } from "@tauri-apps/plugin-store";
import { useCallback, useEffect, useState } from "react";
import { commands } from "../bindings";

// Legacy persisted-store key. The v1 implementation kept the PAT here as
// plain JSON; v2 moves it into the OS keychain via Rust `keyring`. We
// migrate-and-delete on first read so an existing user doesn't have to
// re-enter their token.
const LEGACY_STORE_FILE = "projectpacker.settings.json";
const LEGACY_TOKEN_KEY = "github-token";

const legacyStore = new LazyStore(LEGACY_STORE_FILE);

async function migrateLegacyToken(): Promise<void> {
  try {
    const legacy = await legacyStore.get<string>(LEGACY_TOKEN_KEY);
    if (legacy && legacy.length > 0) {
      // Best-effort: try to move it into the keychain. If the format is
      // bad (older builds were lax about validation) we just drop it
      // — the user can re-paste a valid one.
      const res = await commands.githubSetToken(legacy);
      if (res.status === "ok") {
        await legacyStore.delete(LEGACY_TOKEN_KEY);
        await legacyStore.save();
      }
    }
  } catch {
    // Migration is best-effort — don't crash the hook if anything in the
    // chain fails.
  }
}

let migrationPromise: Promise<void> | null = null;
function ensureMigrated(): Promise<void> {
  if (migrationPromise === null) {
    migrationPromise = migrateLegacyToken();
  }
  return migrationPromise;
}

/**
 * Status-only hook over the OS keychain. The actual token never reaches
 * JS — `setToken`/`clearToken` round-trip through Rust commands and only
 * `hasToken` (boolean) is observable from the renderer.
 *
 * Returns:
 *   - `hasToken`: whether the keychain has a stored PAT.
 *   - `ready`: false until the first status read completes (and any
 *     legacy-store migration finishes). Components that conditionally
 *     fetch (GithubConnector loading repos) should gate on this.
 *   - `setToken(t)`: stores `t` in the keychain. Throws if the PAT
 *     format is rejected by the Rust validator.
 *   - `clearToken()`: deletes the keychain entry. Idempotent.
 *   - `refresh()`: re-read status (used after manual changes elsewhere).
 */
export interface UseGithubTokenReturn {
  hasToken: boolean;
  ready: boolean;
  setToken: (t: string) => Promise<void>;
  clearToken: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useGithubToken(): UseGithubTokenReturn {
  const [hasToken, setHasToken] = useState(false);
  const [ready, setReady] = useState(false);

  const refresh = useCallback(async () => {
    const res = await commands.githubTokenStatus();
    if (res.status === "ok") {
      setHasToken(res.data);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    // Read keychain status first — this is what gates the UI. We do NOT
    // wait for the legacy-store migration before flipping `ready`,
    // because if the LazyStore takes a long time to initialize (or fails
    // to resolve at all), the Settings input + Save button would stay
    // disabled forever. The migration runs in parallel below and
    // re-refreshes status if it moves anything.
    (async () => {
      await refresh();
      if (cancelled) return;
      setReady(true);
    })();
    void ensureMigrated().then(() => {
      if (!cancelled) void refresh();
    });
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  const setToken = useCallback(
    async (t: string) => {
      const res = await commands.githubSetToken(t);
      if (res.status !== "ok") {
        // Surface the Rust error back to the caller — Settings.tsx wraps
        // this so the UI shows the right message.
        throw new Error(res.error.message);
      }
      await refresh();
    },
    [refresh],
  );

  const clearToken = useCallback(async () => {
    const res = await commands.githubClearToken();
    if (res.status !== "ok") {
      throw new Error(res.error.message);
    }
    await refresh();
  }, [refresh]);

  return { hasToken, ready, setToken, clearToken, refresh };
}
