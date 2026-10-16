import { call } from "@/lib/tauri-client";
import type { ReflogEntry } from "./types";

/** Reflog of "HEAD" or a local branch name, newest first. */
export const getReflog = (repoId: string, refName: string) =>
  call<ReflogEntry[]>("get_reflog", { repoId, refName });
