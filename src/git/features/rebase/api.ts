import { call } from "@/lib/tauri-client";
import type { ConflictResolution, RebaseStatus } from "./types";

export const startRebase = (repoId: string, ontoBranch: string) =>
  call<RebaseStatus>("start_rebase", { repoId, ontoBranch });

export const getRebaseStatus = (repoId: string) =>
  call<RebaseStatus | null>("get_rebase_status", { repoId });

export const resolveRebaseConflict = (
  repoId: string,
  path: string,
  resolution: ConflictResolution,
) => call<RebaseStatus>("resolve_rebase_conflict", { repoId, path, resolution });

export const continueRebase = (repoId: string) =>
  call<RebaseStatus>("continue_rebase", { repoId });

export const abortRebase = (repoId: string) => call<void>("abort_rebase", { repoId });
