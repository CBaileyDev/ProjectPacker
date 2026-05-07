import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useState } from "react";

export function CopyButton({ label, text }: { label: string; text: string }) {
  const [copied, setCopied] = useState(false);
  async function doCopy() {
    await writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }
  return (
    <button
      type="button"
      onClick={doCopy}
      className="rounded border border-zinc-600 bg-zinc-800 px-4 py-2 text-sm hover:bg-zinc-700 active:scale-95 transition-all"
    >
      {copied ? "✓ Copied!" : label}
    </button>
  );
}
