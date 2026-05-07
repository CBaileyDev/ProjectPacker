export function DropOverlay({ visible }: { visible: boolean }) {
  if (!visible) return null;
  return (
    <div
      // pointer-events-none lets the underlying webview still receive the
      // drop event; the overlay is purely visual.
      className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-emerald-500/10 backdrop-blur-sm"
    >
      <div className="rounded-lg border-2 border-dashed border-emerald-400 bg-zinc-900/90 px-8 py-6 text-lg font-semibold text-emerald-300 shadow-2xl">
        Drop folder to pack
      </div>
    </div>
  );
}
