export function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2 group">
      <input
        type="checkbox"
        className="mt-0.5 h-4 w-4 shrink-0 rounded accent-emerald-500"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>
        <span className="text-sm text-zinc-200 group-hover:text-white">
          {label}
        </span>
        {hint && <span className="ml-1.5 text-xs text-zinc-500">{hint}</span>}
      </span>
    </label>
  );
}
