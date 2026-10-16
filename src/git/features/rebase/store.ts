import { create } from "zustand";
import type { RebaseStatus } from "./types";

type RebaseStore = {
  status: RebaseStatus | null;
  setStatus: (status: RebaseStatus | null) => void;
};

export const useRebaseStore = create<RebaseStore>((set) => ({
  status: null,
  setStatus: (status) => set({ status }),
}));
