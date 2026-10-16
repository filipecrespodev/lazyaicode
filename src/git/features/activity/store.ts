import { create } from "zustand";
import { getConfig, setConfig } from "@/lib/config-store";
import { type ActivityEntry, appendEntry } from "./format";

export type PinSide = "left" | "right";

type ActivityStore = {
  entries: ActivityEntry[];
  open: boolean;
  /** Docked beside the app instead of shown as a modal. */
  pin: PinSide | null;
  add: (entry: Omit<ActivityEntry, "id" | "at">) => number;
  finish: (id: number, patch: Partial<ActivityEntry>) => void;
  clear: (keep: (e: ActivityEntry) => boolean) => void;
  setOpen: (open: boolean) => void;
  toggle: () => void;
  setPin: (pin: PinSide | null) => void;
};

let nextId = 1;

export const useActivityStore = create<ActivityStore>((set) => ({
  entries: [],
  open: false,
  pin: null,
  add: (entry) => {
    const id = nextId++;
    set((s) => ({ entries: appendEntry(s.entries, { ...entry, id, at: Date.now() }) }));
    return id;
  },
  finish: (id, patch) => set((s) => ({ entries: s.entries.map((e) => (e.id === id ? { ...e, ...patch } : e)) })),
  clear: (keep) => set((s) => ({ entries: s.entries.filter(keep) })),
  setOpen: (open) => set({ open }),
  toggle: () => set((s) => ({ open: !s.open })),
  setPin: (pin) => {
    setConfig("activityLogPin", pin);
    set({ pin });
  },
}));

/** Restores a pinned panel from last session (so it stays in view). */
export async function initActivityLog() {
  const pin = await getConfig<PinSide | null>("activityLogPin", null);
  if (pin) useActivityStore.setState({ pin, open: true });
}

// Repo ids are per session (a tab unloaded for memory gets a new one when
// reopened), so entries are keyed by the repo's path, learned on open.
const repoPaths = new Map<string, string>();
export const rememberRepoPath = (repoId: string, path: string) => repoPaths.set(repoId, path);
export const repoPathOf = (repoId: string) => repoPaths.get(repoId);
