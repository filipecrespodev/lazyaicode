import { call } from "@/lib/tauri-client";

// A IA de commit roda pelo Claude Code com a conta do workspace aberto (o
// backend resolve qual): aqui só vão o modelo e as instruções.

export type CommitMessageResult = {
  summary: string;
  description: string;
};

export function generateCommitMessage(args: {
  model: string;
  diff: string;
  customInstructions: string;
}): Promise<CommitMessageResult> {
  return call<CommitMessageResult>("ai_generate_commit_message", args);
}

/** `changes` are staged-change ids: `path#N` (one hunk) or `path` (whole file). */
export type ComposedCommit = { summary: string; description: string; changes: string[] };

/** Either a commit plan, or (when the model needs clarification) questions. */
export type ComposeResult = { commits: ComposedCommit[]; questions: string[]; treeId: string };

export function composeCommits(args: {
  repoId: string;
  model: string;
  customInstructions: string;
  answers: string;
}): Promise<ComposeResult> {
  return call<ComposeResult>("ai_compose_commits", args);
}

export const applyComposedCommits = (
  repoId: string,
  treeId: string,
  commits: ComposedCommit[],
  authorName: string | null,
  authorEmail: string | null,
  noVerify = false,
) => call<string[]>("apply_composed_commits", { repoId, treeId, commits, authorName, authorEmail, noVerify });

export function resolveConflictWithAi(args: {
  model: string;
  path: string;
  ancestor: string | null;
  ours: string;
  theirs: string;
  customInstructions: string;
}): Promise<string> {
  return call<string>("ai_resolve_conflict", args);
}

export function generatePrDescription(args: {
  model: string;
  commits: string[];
  diff: string;
}): Promise<CommitMessageResult> {
  return call<CommitMessageResult>("ai_generate_pr_description", args);
}
