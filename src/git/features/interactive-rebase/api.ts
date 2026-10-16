import type { CommitNode } from "@/features/commit-graph/types";
import type { RebaseStatus } from "@/features/rebase/types";
import { call } from "@/lib/tauri-client";
import type { TodoItem } from "./todo";

export type RebasePlan = {
  /** null: rewrites from the root commit. */
  upstream: string | null;
  /** Oldest first. */
  commits: CommitNode[];
};

export const getRebasePlan = (repoId: string, fromSha: string) =>
  call<RebasePlan>("get_rebase_plan", { repoId, fromSha });

export const startInteractiveRebase = (repoId: string, upstream: string | null, items: TodoItem[]) =>
  call<RebaseStatus>("start_interactive_rebase", { repoId, upstream, items });
