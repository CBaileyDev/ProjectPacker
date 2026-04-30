import { useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { commands } from "../bindings";

export default function Bridge() {
  const [plan, setPlan] = useState("");
  const [errors, setErrors] = useState<{ code: string; message: string }[]>([]);
  const [combined, setCombined] = useState<string | null>(null);

  async function check() {
    setErrors([]); setCombined(null);
    const v = await commands.validatePlan(plan, "grok-to-cc-v1");
    if (v.status !== "ok") return;
    if (!v.data.ok) { setErrors(v.data.errors); return; }
    const w = await commands.buildCombinedPrompt(plan, "grok-to-cc-v1");
    if (w.status === "ok") setCombined(w.data);
  }

  return (
    <div className="space-y-4">
      <h1 className="text-2xl">Bridge</h1>
      <textarea
        className="h-64 w-full rounded bg-zinc-800 p-2 font-mono text-sm"
        value={plan}
        onChange={(e) => setPlan(e.target.value)}
        placeholder="Paste Grok's plan here…"
      />
      <button className="rounded bg-emerald-700 px-4 py-2 hover:bg-emerald-600" onClick={check}>
        Validate & Build Prompt
      </button>
      {errors.length > 0 && (
        <div className="rounded border border-red-600 bg-red-950 p-2 text-sm">
          {errors.map((e, i) => <div key={i}>• {e.message}</div>)}
        </div>
      )}
      {combined && (
        <div className="space-y-2">
          <button className="rounded bg-zinc-700 px-3 py-1" onClick={() => writeText(combined)}>Copy Combined Prompt</button>
          <pre className="max-h-64 overflow-auto rounded bg-zinc-900 p-2 text-xs">{combined}</pre>
        </div>
      )}
    </div>
  );
}
