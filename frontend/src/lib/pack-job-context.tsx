import { createContext, type ReactNode, useContext } from "react";
import { type UsePackJobReturn, usePackJob } from "./use-pack-job";

const PackJobContext = createContext<UsePackJobReturn | null>(null);

/** Owns the single pack-job lifecycle. Mounted once in the always-mounted
 * shell, so moment components can mount/unmount freely mid-run without
 * detaching the progress channel or losing the error banner state. */
export function PackJobProvider({ children }: { children: ReactNode }) {
  const job = usePackJob();
  return (
    <PackJobContext.Provider value={job}>{children}</PackJobContext.Provider>
  );
}

export function usePackJobContext(): UsePackJobReturn {
  const ctx = useContext(PackJobContext);
  if (!ctx) {
    throw new Error("usePackJobContext must be used inside PackJobProvider");
  }
  return ctx;
}
