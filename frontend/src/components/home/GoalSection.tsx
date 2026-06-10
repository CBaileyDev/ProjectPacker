import { useApp } from "../../lib/store";
import { SectionTitle } from "./SectionTitle";

export function GoalSection() {
  const goal = useApp((s) => s.options.goal);
  const patchOptions = useApp((s) => s.patchOptions);
  return (
    <section className="space-y-3">
      <SectionTitle>Goal / Task Description</SectionTitle>
      <textarea
        className="h-20 w-full resize-none rounded-lg border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 transition-colors focus:border-primary/50 focus:outline-none"
        value={goal}
        placeholder="Describe what you want to build or fix…"
        aria-label="Goal or task description"
        onChange={(e) => patchOptions({ goal: e.target.value })}
      />
    </section>
  );
}
