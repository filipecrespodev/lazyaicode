/** Default spot for a new worktree: a sibling of the repo folder named
 * `<repo>-<branch>` (slashes in the branch flattened), using the repo
 * path's own separator. */
export function suggestWorktreePath(repoPath: string, branch: string): string {
  const sep = repoPath.includes("\\") && !repoPath.includes("/") ? "\\" : "/";
  const trimmed = repoPath.replace(/[\\/]+$/, "");
  const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  const parent = cut >= 0 ? trimmed.slice(0, cut) : "";
  const repoName = cut >= 0 ? trimmed.slice(cut + 1) : trimmed;
  const slug = branch
    .trim()
    .replace(/[\\/:*?"<>|\s]+/g, "-")
    .replace(/^-+|-+$/g, "");
  const name = slug ? `${repoName}-${slug}` : `${repoName}-worktree`;
  return parent ? `${parent}${sep}${name}` : name;
}
