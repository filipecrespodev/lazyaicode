import { call } from "@/lib/tauri-client";
import type { RepoStats } from "./types";

/** Statistics over the checked-out branch's history (everything reachable from HEAD). */
export const getRepoStats = (repoId: string) => call<RepoStats>("get_repo_stats", { repoId });
