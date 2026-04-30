import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";
import type { ProgressEvent } from "./api";

export function subscribePackProgress(
  jobId: string,
  onEvent: (e: ProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProgressEvent>(`pack:${jobId}:progress`, (e: Event<ProgressEvent>) => onEvent(e.payload));
}
