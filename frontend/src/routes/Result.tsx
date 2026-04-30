import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useApp } from "../lib/store";

export default function Result() {
  const { result } = useApp();
  if (!result) return <div>No result. Go pack something.</div>;
  return (
    <div className="space-y-4">
      <h1 className="text-2xl">Result</h1>
      <div className="text-sm text-zinc-300">
        {result.stats.filesIncluded} files · {result.stats.bytesTotal} bytes
        {result.stats.tokensTotal != null && <> · {result.stats.tokensTotal} tokens</>}
      </div>
      <div className="flex gap-2">
        <button className="rounded bg-zinc-700 px-3 py-1" onClick={() => writeText(result.xml)}>Copy Pack XML</button>
        <button className="rounded bg-zinc-700 px-3 py-1" onClick={() => writeText(result.claudeCodePrompt)}>Copy Claude Code Prompt</button>
      </div>
      <details>
        <summary className="cursor-pointer">Pack XML preview</summary>
        <pre className="max-h-96 overflow-auto rounded bg-zinc-900 p-2 text-xs">{result.xml}</pre>
      </details>
    </div>
  );
}
