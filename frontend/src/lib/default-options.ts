import type { PackOptions } from "../bindings";
import { PROTOCOL_VERSION } from "./protocol";

/** Factory defaults for a fresh install. Kept in a pure module (no zustand,
 * no Tauri imports) so unit tests can assert invariants against it — e.g.
 * that a fresh install's options read as the "Balanced" preset. */
export const defaultOptions: PackOptions = {
  target: { kind: "folder", value: "" },
  goal: "",
  countTokens: true,
  tokenizerModel: "gpt-4o-mini",
  secretScan: true,
  compress: false,
  removeComments: false,
  // Lossless 4 (default on — match Rust `PackOptions::default`).
  dedupFiles: true,
  trimTrailingWs: true,
  collapseBlankLines: true,
  normalizeLineEndings: true,
  // Semantic 3 + TS type-only elider (default off — opt in).
  collapseLockfiles: false,
  collapseMinified: false,
  markGenerated: false,
  elideTypeOnlyExports: false,
  maxFileSizeKb: 1024,
  respectGitignore: true,
  customIgnorePatterns: [],
  protocolVersion: PROTOCOL_VERSION,
  format: "xml",
  xmlSchema: "cxml",
};
