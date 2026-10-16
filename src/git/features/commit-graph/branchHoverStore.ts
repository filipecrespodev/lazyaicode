import { create } from "zustand";

/** Tip commit of the branch pill under the pointer: the graph fades every
 * commit that isn't reachable from it. */
export const useBranchHoverStore = create<{
  tipSha: string | null;
  set: (tipSha: string | null) => void;
}>((set) => ({ tipSha: null, set: (tipSha) => set({ tipSha }) }));
