import { BridgeTab } from "../components/bridge/BridgeTab";
import { useApp, usePackProgress } from "../lib/store";

/** Final stage of the journey: paste plan → validate → executor prompt.
 * Wraps the existing BridgeTab (logic + tests unchanged). */
export default function Bridge() {
  const setMoment = useApp((s) => s.setMoment);
  const { result } = usePackProgress();

  return (
    // Plain div: MomentView's wrapper is the single entrance-animation
    // owner — a route-level fadeUp on top compounds into a double slide.
    <div className="mx-auto w-full max-w-3xl space-y-5 px-6 pb-16 pt-8">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-primary">
            Bridge
          </p>
          <h1 className="mt-1 text-2xl font-bold tracking-tight">
            Validate a plan, build the executor prompt
          </h1>
        </div>
        <button
          type="button"
          onClick={() => setMoment(result ? "results" : "home")}
          className="text-sm text-zinc-400 underline-offset-2 transition-colors hover:text-zinc-100 hover:underline"
        >
          ← back
        </button>
      </div>
      <BridgeTab />
    </div>
  );
}
