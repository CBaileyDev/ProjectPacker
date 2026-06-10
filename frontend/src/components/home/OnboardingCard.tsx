import * as m from "framer-motion/m";
import type { LucideProps } from "lucide-react";
import type { ComponentType } from "react";
import { prefersReducedMotion } from "../../lib/motion";
import {
  FileTextIcon,
  FolderIcon,
  PackageIcon,
  SparklesIcon,
} from "../pack/icons";

// ─────────────────────────────────────────────────────────────────────────
// Onboarding empty state — shown on the Packer tab when no target is
// selected and nothing has run yet. Walks through the 3-step flow.
// ─────────────────────────────────────────────────────────────────────────

const ONBOARDING_STEPS: Array<{
  Icon: ComponentType<LucideProps>;
  title: string;
  body: string;
}> = [
  {
    Icon: FolderIcon,
    title: "Pick a target",
    body: "Choose a local folder, drop one onto the window, or paste a GitHub repo URL.",
  },
  {
    Icon: FileTextIcon,
    title: "Describe your goal",
    body: "Say what you want to build or fix — it's embedded in the pack for the AI.",
  },
  {
    Icon: PackageIcon,
    title: "Pack",
    body: "One AI-ready file with token counts, secret scanning, and compression stats.",
  },
];

export function OnboardingCard() {
  return (
    <m.section
      aria-label="Getting started"
      className="rounded-2xl border border-dashed border-zinc-700/70 bg-zinc-900/25 p-6"
      initial={prefersReducedMotion ? false : { opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0 }}
      transition={
        prefersReducedMotion
          ? { duration: 0 }
          : { duration: 0.3, ease: [0.22, 1, 0.36, 1] }
      }
    >
      <div className="flex items-center gap-2">
        <SparklesIcon size={15} className="text-primary" />
        <h3 className="text-sm font-semibold text-zinc-200">
          Get started in three steps
        </h3>
      </div>
      <ol className="mt-4 grid gap-3 md:grid-cols-3">
        {ONBOARDING_STEPS.map(({ Icon, title, body }, i) => (
          <li
            key={title}
            className="rounded-xl border border-zinc-800 bg-zinc-950/50 p-4"
          >
            <div className="flex items-center gap-2.5">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 ring-1 ring-primary/20">
                <Icon size={15} className="text-primary" />
              </span>
              <span className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                Step {i + 1}
              </span>
            </div>
            <p className="mt-3 text-sm font-semibold text-zinc-200">{title}</p>
            <p className="mt-1 text-xs leading-relaxed text-zinc-500">{body}</p>
          </li>
        ))}
      </ol>
    </m.section>
  );
}
