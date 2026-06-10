import type { TokensPerModel } from "../../bindings";
import { fmtNum } from "../../lib/format";
import { type Fit, fitsIn, MODEL_WINDOWS } from "../../lib/model-windows";

const FIT_STYLES: Record<Fit, { label: string; cls: string }> = {
  fits: { label: "✓ fits", cls: "text-success" },
  tight: { label: "◐ tight", cls: "text-warning" },
  over: { label: "✗ over", cls: "text-red-400" },
};

/** "Fits in" table: per-model token count vs advertised context window.
 * Replaces the old AiContextTable. Hidden entirely when token counting
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
        <tbody>
          {MODEL_WINDOWS.map(({ key, label, window }) => {
            const tokens = tokensPerModel[key];
            const fit = fitsIn(tokens, window);
            const style = FIT_STYLES[fit];
            return (
              <tr key={key} className="border-b border-white/5 last:border-0">
                <td className="py-2 text-zinc-200">
                  {label}{" "}
                  <span className={`ml-1.5 text-xs ${style.cls}`}>
                    {style.label}
                  </span>
                </td>
                <td className="nums py-2 text-right font-mono text-xs text-zinc-400">
                  {fmtNum(tokens)} / {fmtNum(window)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
