import type { TokensPerModel } from "../../bindings";
import { AI_MODELS } from "../../lib/ai-models";
import { fmtNum } from "../../lib/format";

export function AiContextTable({
  tokensPerModel,
}: {
  tokensPerModel: TokensPerModel | null;
}) {
  if (!tokensPerModel) {
    return (
      <div className="rounded border border-zinc-700 bg-zinc-800/50 p-4 text-sm text-zinc-400">
        Enable “Count tokens” in options to see AI context-window compatibility.
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded border border-zinc-700">
      <div className="border-b border-zinc-700 bg-zinc-800 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">
        AI Context Window Compatibility
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-zinc-700 text-xs text-zinc-500">
            <th className="px-3 py-2 text-left font-normal">Model</th>
            <th className="px-3 py-2 text-right font-normal">
              Tokens / context
            </th>
            <th className="px-3 py-2 text-center font-normal">Fits?</th>
          </tr>
        </thead>
        <tbody>
          {AI_MODELS.map((m) => {
            const tokens = tokensPerModel[m.tokenModel];
            const fits = tokens <= m.context;
            const pct = Math.min(100, Math.round((tokens / m.context) * 100));
            return (
              <tr
                key={m.name}
                className="border-b border-zinc-800 last:border-0"
              >
                <td className="px-3 py-2 text-zinc-200">
                  {m.name}
                  {m.approx && (
                    <span className="ml-1.5 rounded bg-zinc-700 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-zinc-300">
                      approx
                    </span>
                  )}
                </td>
                <td className="px-3 py-2 text-right text-zinc-400">
                  <span className="font-medium text-zinc-200">
                    {fmtNum(tokens)}
                  </span>
                  <span className="text-zinc-500"> / {fmtNum(m.context)}</span>
                </td>
                <td className="px-3 py-2">
                  <div className="flex items-center justify-center gap-2">
                    {fits ? (
                      <span className="font-medium text-emerald-400">
                        ✓ Yes
                      </span>
                    ) : (
                      <span className="font-medium text-red-400">✗ No</span>
                    )}
                    <span className="text-xs text-zinc-500">({pct}%)</span>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <div className="border-t border-zinc-700 bg-zinc-800/50 px-3 py-2 text-xs text-zinc-500">
        Rows marked “approx” use a proxy tokenizer (cl100k for Claude/Grok,
        cl100k×1.05 ceil for Gemini) since the authentic tokenizers are not
        public.
      </div>
    </div>
  );
}
