import type { PackOptions } from "../bindings";

/** A preset is a named bundle over existing PackOptions fields. Matching
 * compares ONLY the patch keys, so target/goal/format never disqualify a
 * preset. The chip row derives its highlighted state via matchPreset —
 * the active preset is never stored. */
export interface Preset {
  id: "balanced" | "minimal" | "everything";
  label: string;
  description: string;
  patch: Partial<PackOptions>;
}

export const PRESETS: readonly Preset[] = [
  {
    id: "balanced",
    label: "Balanced",
    description: "Lossless cleanup, full-fidelity code",
    patch: {
      compress: false,
      removeComments: false,
      dedupFiles: true,
      trimTrailingWs: true,
      collapseBlankLines: true,
      normalizeLineEndings: true,
      collapseLockfiles: false,
      collapseMinified: false,
      markGenerated: false,
      elideTypeOnlyExports: false,
      maxFileSizeKb: 1024,
    },
  },
  {
    id: "minimal",
    label: "Minimal tokens",
    description: "Every safe compressor on, small files only",
    patch: {
      compress: true,
      removeComments: true,
      dedupFiles: true,
      trimTrailingWs: true,
      collapseBlankLines: true,
      normalizeLineEndings: true,
      collapseLockfiles: true,
      collapseMinified: true,
      markGenerated: true,
      elideTypeOnlyExports: true,
      maxFileSizeKb: 512,
    },
  },
  {
    id: "everything",
    label: "Everything",
    description: "No transforms — raw files, max size cap",
    patch: {
      compress: false,
      removeComments: false,
      dedupFiles: false,
      trimTrailingWs: false,
      collapseBlankLines: false,
      normalizeLineEndings: false,
      collapseLockfiles: false,
      collapseMinified: false,
      markGenerated: false,
      elideTypeOnlyExports: false,
      maxFileSizeKb: 102_400,
    },
  },
];

export function applyPreset(options: PackOptions, preset: Preset): PackOptions {
  return { ...options, ...preset.patch };
}

export function matchPreset(options: PackOptions): Preset["id"] | null {
  for (const p of PRESETS) {
    const matches = Object.entries(p.patch).every(
      ([k, v]) => options[k as keyof PackOptions] === v,
    );
    if (matches) return p.id;
  }
  return null;
}
