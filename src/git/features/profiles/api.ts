import { call } from "@/lib/tauri-client";

export type GlobalGitIdentity = { name: string | null; email: string | null };

export const getGlobalGitIdentity = () => call<GlobalGitIdentity>("get_global_git_identity");
