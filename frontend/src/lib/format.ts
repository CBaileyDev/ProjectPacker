export function fmtNum(n: number): string {
  return n.toLocaleString();
}

export function fmtBytes(b: number): string {
  if (b >= 1_048_576) return `${(b / 1_048_576).toFixed(1)} MB`;
  if (b >= 1_024) return `${(b / 1_024).toFixed(1)} KB`;
  return `${b} B`;
}

/**
 * Bytes-saved chip formatter shared by the Compression panel summary and
 * the per-transform rows. Deliberately distinct from {@link fmtBytes}:
 * `0` renders as `"0 B"` and the MB branch keeps two decimals so small
 * per-transform savings stay visible.
 */
export function fmtBytesSaved(n: number): string {
  if (n === 0) return "0 B";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}
