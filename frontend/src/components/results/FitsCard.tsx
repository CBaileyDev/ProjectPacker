import type { TokensPerModel } from "../../bindings";
import { fmtNum } from "../../lib/format";
import { type Fit, fitsIn, MODEL_WINDOWS } from "../../lib/model-windows";

const FIT_STYLES: Record<Fit, { label: string; cls: string }> = {
  fits: { label: "✓", cls: "text-success" },
  tight: { label: "◐", cls: "text-warning" },
  over: { label: "✗", cls: "text-red-400" },
};

/** One window cell: fit glyph + the window it was judged against. */
function WindowCell({ tokens, window }: { tokens: number; window: number }) {
  const style = FIT_STYLES[fitsIn(tokens, window)];
  return (
    <td className="nums py-2 text-right font-mono text-xs text-zinc-400">
      <span className={`mr-1.5 ${style.cls}`}>{style.label}</span>
      {fmtNum(window)}
    </td>
  );
}

/** "Fits in" table: per-family token count vs the current flagship's
 * context windows — consumer app and API/coding tools shown separately,
 * because they often differ by 4-5x. Hidden entirely when token counting
 * was disabled for the pack. */
export function FitsCard({
  tokensPerModel,
}: {
  tokensPerModel: TokensPerModel | null;
}) {
  if (!tokensPerModel) return null;
  return (
    <section className="rounded-xl border border-white/10 bg-white/[0.03] p-5">
      <div className="mb-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-zinc-500">
        Fits in
      </div>
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="text-[10px] uppercase tracking-wider text-zinc-600">
            <th scope="col" className="pb-1.5 font-semibold">
              Model
            </th>
            <th scope="col" className="pb-1.5 text-right font-semibold">
              Pack
            </th>
            <th scope="col" className="pb-1.5 text-right font-semibold">
              App
            </th>
            <th scope="col" className="pb-1.5 text-right font-semibold">
              API / Code
            </th>
          </tr>
        </thead>
        <tbody>
          {MODEL_WINDOWS.map(({ key, label, appWindow, apiWindow, hint }) => {
            const tokens = tokensPerModel[key];
            return (
              <tr
                key={key}
                title={hint}
                className="border-b border-white/5 last:border-0"
              >
                <th scope="row" className="py-2 font-normal text-zinc-200">
                  {label}
                </th>
                <td className="nums py-2 text-right font-mono text-xs text-zinc-300">
                  {fmtNum(tokens)}
                </td>
                <WindowCell tokens={tokens} window={appWindow} />
                <WindowCell tokens={tokens} window={apiWindow} />
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="mt-2.5 text-[11px] leading-relaxed text-zinc-600">
        Windows verified June 2026; app = paid chat plan, API / Code = developer
        access. Counts are estimates from each family's nearest public tokenizer
        — hover a row for tier details.
      </p>
    </section>
  );
}
