/** Shas reachable from `tipSha` (the tip included) by walking parents through
 * `commits`. Parents outside the loaded list are not followed. */
export function reachableFrom(
  commits: { sha: string; parents: string[] }[],
  tipSha: string,
): Set<string> {
  const bySha = new Map(commits.map((c) => [c.sha, c] as const));
  const seen = new Set<string>();
  const stack = [tipSha];
  while (stack.length) {
    const sha = stack.pop()!;
    if (seen.has(sha) || !bySha.has(sha)) continue;
    seen.add(sha);
    stack.push(...bySha.get(sha)!.parents);
  }
  return seen;
}
