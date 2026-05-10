import { memo } from "react";

/**
 * Loading-state placeholders that share a single CSS shimmer animation
 * (`.skeleton-shimmer` in `globals.css`). Each component is `React.memo`'d
 * because skeletons routinely render in lists and they have no internal
 * state — re-rendering on every parent tick would be pure waste.
 *
 * All four components are `aria-hidden` so screen readers skip the
 * placeholder noise; surrounding live regions should provide the loading
 * status text.
 */

interface SkeletonBlockProps {
  className?: string;
  width?: number | string;
  height?: number | string;
}

export const SkeletonBlock = memo(function SkeletonBlock({
  className,
  width,
  height,
}: SkeletonBlockProps) {
  const style: React.CSSProperties = {};
  if (width !== undefined) style.width = width;
  if (height !== undefined) style.height = height;
  return (
    <div
      aria-hidden="true"
      className={`skeleton-shimmer rounded-md ${className ?? ""}`}
      style={style}
    />
  );
});

interface SkeletonTextProps {
  /** Number of stacked lines. Defaults to 1. */
  lines?: number;
  /** Width of the final line — narrower than full to mimic line-end. */
  lastWidth?: string;
}

export const SkeletonText = memo(function SkeletonText({
  lines = 1,
  lastWidth = "60%",
}: SkeletonTextProps) {
  return (
    <div aria-hidden="true" className="flex flex-col gap-2">
      {Array.from({ length: lines }).map((_, i) => {
        const isLast = i === lines - 1 && lines > 1;
        // Static placeholder list with stable ordering — the index IS
        // the identity, so the lint warning about index keys doesn't
        // apply here.
        return (
          <div
            // biome-ignore lint/suspicious/noArrayIndexKey: stable static placeholder list
            key={i}
            className="skeleton-shimmer h-3 rounded-md"
            style={{ width: isLast ? lastWidth : "100%" }}
          />
        );
      })}
    </div>
  );
});

interface SkeletonCircleProps {
  size?: number;
}

export const SkeletonCircle = memo(function SkeletonCircle({
  size = 32,
}: SkeletonCircleProps) {
  return (
    <div
      aria-hidden="true"
      className="skeleton-shimmer rounded-full"
      style={{ width: size, height: size }}
    />
  );
});

export const SkeletonRow = memo(function SkeletonRow() {
  return (
    <div aria-hidden="true" className="flex items-center gap-3 py-2">
      <SkeletonCircle size={28} />
      <div className="flex-1">
        <SkeletonText lines={2} lastWidth="40%" />
      </div>
    </div>
  );
});
