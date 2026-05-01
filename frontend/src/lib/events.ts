import { Channel } from "@tauri-apps/api/core";
import type { ProgressEvent } from "./api";

export function createPackProgressChannel(
  onEvent: (e: ProgressEvent) => void,
): Channel<ProgressEvent> {
  const channel = new Channel<ProgressEvent>();
  channel.onmessage = onEvent;
  return channel;
}
