import { call } from "@/lib/tauri-client";
import { t } from "../../../lib/i18n/react";

/** Mirrors `CommitSignature` in src-tauri/src/git/signing.rs. */
export type CommitSignature = { format: "ssh" | "gpg" | "x509" | "unknown" };

/** `null` for an unsigned commit. Presence only: not verified. */
export const getCommitSignature = (repoId: string, sha: string) =>
  call<CommitSignature | null>("get_commit_signature", { repoId, sha });

const FORMAT_NAMES: Partial<Record<CommitSignature["format"], string>> = {
  ssh: "SSH",
  gpg: "GPG",
  x509: "X.509",
};

export function signatureLabel(signature: CommitSignature | null | undefined): string | null {
  if (!signature) return null;
  const format = FORMAT_NAMES[signature.format];
  return format ? t("gitDiff.signedWith", { format }) : t("gitDiff.signed");
}
