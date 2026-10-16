import { load, type Store } from "@tauri-apps/plugin-store";

let storePromise: Promise<Store> | null = null;

function getStore(): Promise<Store> {
  if (!storePromise) storePromise = load("layout.json", { autoSave: true });
  return storePromise;
}

export async function getConfig<T>(key: string, fallback: T): Promise<T> {
  const store = await getStore();
  const value = await store.get<T>(key);
  return value ?? fallback;
}

export async function setConfig<T>(key: string, value: T): Promise<void> {
  const store = await getStore();
  await store.set(key, value);
}
