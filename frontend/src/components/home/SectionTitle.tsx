import type React from "react";

export function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="block text-xs font-semibold uppercase tracking-wider text-zinc-500">
      {children}
    </h3>
  );
}
