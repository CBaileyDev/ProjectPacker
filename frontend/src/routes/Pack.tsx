import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { commands } from "../bindings";
import { useApp } from "../lib/store";
import { subscribePackProgress } from "../lib/events";

export default function Pack() {
  const nav = useNavigate();
  const { options, setOptions, status, events, setJob, pushEvent, setResult } = useApp();
  const [busy, setBusy] = useState(false);

  async function pickFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      setOptions({ ...options, target: { kind: "folder", value: path } });
    }
  }

  async function runPack() {
    setBusy(true);
    const start = await commands.packStart(options);
    if (start.status === "ok") {
      const jobId = start.data;
      setJob(jobId);
      const unlisten = await subscribePackProgress(jobId, (e) => {
        pushEvent(e);
        if (e.kind === "done") {
          (async () => {
            const r = await commands.packGetResult(jobId);
            if (r.status === "ok") setResult(r.data);
            unlisten();
            nav("/result");
          })();
        }
      });
    }
    setBusy(false);
  }

  const targetVal = options.target.kind === "folder" ? options.target.value : "";

  return (
    <div className="space-y-4">
      <h1 className="text-2xl">Pack</h1>
      <div className="space-y-2">
        <label className="block text-sm">Target folder</label>
        <div className="flex gap-2">
          <input
            className="flex-1 rounded bg-zinc-800 px-2 py-1"
            value={targetVal}
            onChange={(e) => setOptions({ ...options, target: { kind: "folder", value: e.target.value } })}
          />
          <button className="rounded bg-zinc-700 px-3 py-1" onClick={pickFolder}>Browse…</button>
        </div>
      </div>
      <div>
        <label className="block text-sm">Goal</label>
        <textarea
          className="h-24 w-full rounded bg-zinc-800 p-2"
          value={options.goal}
          onChange={(e) => setOptions({ ...options, goal: e.target.value })}
        />
      </div>
      <button
        className="rounded bg-emerald-700 px-4 py-2 hover:bg-emerald-600 disabled:opacity-50"
        onClick={runPack}
        disabled={busy || !targetVal}
      >
        {busy ? "Packing…" : "Pack"}
      </button>
      {status === "running" && (
        <pre className="max-h-64 overflow-auto rounded bg-zinc-900 p-2 text-xs">
          {events.map((e, i) => <div key={i}>{JSON.stringify(e)}</div>)}
        </pre>
      )}
    </div>
  );
}
