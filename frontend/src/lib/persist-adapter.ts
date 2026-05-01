import { LazyStore } from "@tauri-apps/plugin-store";
import type { StateStorage } from "zustand/middleware";

const STORE_FILE = "projectpacker.settings.json";
const lazyStore = new LazyStore(STORE_FILE);

export const tauriStoreAdapter: StateStorage = {
  async getItem(name) {
    const value = await lazyStore.get<string>(name);
    return value ?? null;
  },
  async setItem(name, value) {
    await lazyStore.set(name, value);
    await lazyStore.save();
  },
  async removeItem(name) {
    await lazyStore.delete(name);
    await lazyStore.save();
  },
};
