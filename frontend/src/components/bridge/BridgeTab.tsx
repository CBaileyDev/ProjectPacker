import * as m from "framer-motion/m";
import { useState } from "react";
import { commands, type PlanError } from "../../bindings";
import { fadeUp, springButton } from "../../lib/motion";
import { PROTOCOL_VERSION } from "../../lib/protocol";
import { useApp } from "../../lib/store";
import { CopyButton } from "../pack/CopyButton";
import { AlertIcon, CheckIcon, LoaderIcon } from "../pack/icons";

type BridgeStatus =
  | { kind: "idle" }
  | { kind: "validating" }
  | { kind: "valid"; combinedPrompt: string }
  | { kind: "invalid"; errors: PlanError[] }
  | { kind: "error"; message: string };

/** Re-prompt the user pastes back to the planner when validation fails.
 * Wording follows the design doc's Bridge flow: name the errors, demand a
 * re-emit against the exact protocol version. */
function buildReprompt(errors: PlanError[]): string {
  const list = errors.map((e) => `- ${e.code}: ${e.message}`).join("\n");
  return `Your previous response failed validation. Errors:\n${list}\nPlease re-emit following protocol ${PROTOCOL_VERSION} exactly.`;
}

/**
 * Bridge tab — the validated handoff between planner and executor.
 *
 * Paste a plan produced by the planner AI, validate it against the
 * protocol grammar, and either copy the combined executor prompt (plan
 * wrapped in the executor template) or copy a re-prompt that sends the
 * validator's errors back to the planner.
 *
 * The combined prompt is built eagerly on successful validation so the
 * success state can hand a plain string to `CopyButton`.
 */
export function BridgeTab() {
  const planMd = useApp((s) => s.bridgePlanMd);
  const setPlanMd = useApp((s) => s.setBridgePlanMd);
  const [status, setStatus] = useState<BridgeStatus>({ kind: "idle" });

  const canValidate = planMd.trim().length > 0 && status.kind !== "validating";

  async function validate() {
    setStatus({ kind: "validating" });
    try {
      const v = await commands.validatePlan(planMd, PROTOCOL_VERSION);
      if (v.status === "error") {
        setStatus({ kind: "error", message: v.error.message });
        return;
      }
      if (!v.data.ok) {
        setStatus({ kind: "invalid", errors: v.data.errors });
        return;
      }
      const combined = await commands.buildCombinedPrompt(
        planMd,
        PROTOCOL_VERSION,
      );
      if (combined.status === "error") {
        setStatus({ kind: "error", message: combined.error.message });
        return;
      }
      setStatus({ kind: "valid", combinedPrompt: combined.data });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setStatus({ kind: "error", message });
    }
  }

  return (
    <m.div
      className="space-y-5"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <p className="text-sm text-zinc-400">
        Paste the plan your planner AI produced. ProjectPacker validates it
        against protocol{" "}
        <span className="font-mono text-zinc-300">{PROTOCOL_VERSION}</span>{" "}
        before it ever reaches your executor agent — malformed plans get a
        ready-made re-prompt instead of a blind execution.
      </p>

      <textarea
        value={planMd}
        onChange={(e) => {
          setPlanMd(e.target.value);
          // Stale validation results must not survive edits.
          if (status.kind !== "idle") setStatus({ kind: "idle" });
        }}
        placeholder={
          "### Summary\n…\n\n### Risks\n…\n\n### Steps\n…\n\n### Verification\n…\n\n### Rollback\n…"
        }
        aria-label="Plan markdown"
        spellCheck={false}
        className="h-64 w-full resize-y rounded-lg border border-zinc-700 bg-zinc-800/60 px-3.5 py-3 font-mono text-xs text-zinc-200 placeholder:text-zinc-600 focus:border-emerald-500/50 focus:outline-none"
      />

      <div className="flex flex-wrap items-center gap-2.5">
        <m.button
          type="button"
          disabled={!canValidate}
          onClick={validate}
          whileTap={springButton}
          className="flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-emerald-500 disabled:cursor-not-allowed disabled:bg-zinc-700 disabled:text-zinc-500"
        >
          {status.kind === "validating" ? (
            <>
              <LoaderIcon size={15} className="animate-spin" />
              Validating…
            </>
          ) : (
            "Validate plan"
          )}
        </m.button>

        {status.kind === "valid" && (
          <CopyButton
            label="Copy combined prompt"
            text={status.combinedPrompt}
          />
        )}
        {status.kind === "invalid" && (
          <CopyButton
            label="Copy re-prompt"
            text={buildReprompt(status.errors)}
          />
        )}
      </div>

      {status.kind === "valid" && (
        <m.div
          className="flex items-center gap-2 rounded-lg border border-emerald-600/50 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-400"
          variants={fadeUp}
          initial="hidden"
          animate="visible"
        >
          <CheckIcon size={15} />
          Plan is valid. Copy the combined prompt and paste it into your
          executor agent.
        </m.div>
      )}

      {status.kind === "invalid" && (
        <m.div
          className="space-y-2 rounded-lg border border-red-600/50 bg-red-500/10 px-4 py-3"
          variants={fadeUp}
          initial="hidden"
          animate="visible"
        >
          <div className="flex items-center gap-2 text-sm font-medium text-red-400">
            <AlertIcon size={15} />
            Plan failed validation ({status.errors.length}{" "}
            {status.errors.length === 1 ? "error" : "errors"})
          </div>
          <ul className="space-y-1 text-sm text-red-300/90">
            {status.errors.map((e) => (
              <li key={`${e.code}:${e.message}`} className="flex gap-2">
                <span className="shrink-0 font-mono text-xs text-red-400/80">
                  {e.code}
                </span>
                <span>{e.message}</span>
              </li>
            ))}
          </ul>
          <p className="text-xs text-zinc-400">
            Copy the re-prompt above and paste it back to your planner — it
            names every violation and demands a corrected plan.
          </p>
        </m.div>
      )}

      {status.kind === "error" && (
        <m.div
          className="flex items-center gap-2 rounded-lg border border-red-600/50 bg-red-500/10 px-4 py-3 text-sm text-red-400"
          variants={fadeUp}
          initial="hidden"
          animate="visible"
        >
          <AlertIcon size={15} />
          {status.message}
        </m.div>
      )}
    </m.div>
  );
}
