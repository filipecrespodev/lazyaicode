import { call } from "@/lib/tauri-client";
import type { StashInfo, StashPopResult } from "./types";

export const stashSave = (repoId: string, message: string | null, includeUntracked: boolean) =>
  call<StashInfo>("stash_save", { repoId, message, includeUntracked });

export const stashList = (repoId: string) => call<StashInfo[]>("stash_list", { repoId });

export const stashPop = (repoId: string, index: number) =>
  call<StashPopResult>("stash_pop", { repoId, index });

export const stashApply = (repoId: string, index: number) =>
  call<StashPopResult>("stash_apply", { repoId, index });

export const stashDrop = (repoId: string, index: number) =>
  call<void>("stash_drop", { repoId, index });
