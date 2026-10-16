import { call } from "@/lib/tauri-client";

export type LfsStatus = { usesLfs: boolean; installed: boolean };

export const getLfsStatus = (repoId: string) => call<LfsStatus>("get_lfs_status", { repoId });

export const runLfsPull = (repoId: string) => call<void>("run_lfs_pull", { repoId });
