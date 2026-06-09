/** Default cap for long warning/redaction lists in the Results tab. */
export const DEFAULT_LIST_CAP = 50;

export interface ClampedList<T> {
  /** The items to render right now. */
  visible: T[];
  /** How many items are hidden behind the "Show N more" button. 0 when
   * the list fits under the cap or is expanded. */
  hiddenCount: number;
  /** True when the list exceeds the cap — i.e. a show-more/show-less
   * toggle should be rendered at all. */
  isCapped: boolean;
}

/**
 * Cap a list to its first `cap` entries unless `expanded`. Pure helper so
 * the Results tab's pagination logic is unit-testable without rendering.
 */
export function clampList<T>(
  items: T[],
  expanded: boolean,
  cap: number = DEFAULT_LIST_CAP,
): ClampedList<T> {
  const isCapped = items.length > cap;
  if (!isCapped || expanded) {
    return { visible: items, hiddenCount: 0, isCapped };
  }
  return {
    visible: items.slice(0, cap),
    hiddenCount: items.length - cap,
    isCapped,
  };
}
