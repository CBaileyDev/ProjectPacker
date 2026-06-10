import type { PackFormat } from "../bindings";

export const FORMAT_LABELS: Record<PackFormat, string> = {
  xml: "XML  (planner / executor)",
  markdown: "Markdown",
  plainText: "Plain Text",
};

export const COPY_BUTTON_LABELS: Record<PackFormat, string> = {
  xml: "Copy Pack XML",
  markdown: "Copy Pack Markdown",
  plainText: "Copy Plain Text",
};

export const SAVE_FILENAMES: Record<PackFormat, string> = {
  xml: "pack.xml",
  markdown: "pack.md",
  plainText: "pack.txt",
};

export const GITHUB_URL_PATTERN =
  /^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+(\.git)?\/?$/;

export const MAX_FILE_SIZE_KB = 102_400;

export function isValidTargetValue(
  kind: "folder" | "github",
  value: string,
): boolean {
  return kind === "folder" ? value.length > 0 : GITHUB_URL_PATTERN.test(value);
}
