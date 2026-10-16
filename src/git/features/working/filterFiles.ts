/** Files whose path contains `query` (case-insensitive); everything for a blank query. */
export function filterFiles<T extends { path: string }>(files: T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  return q ? files.filter((f) => f.path.toLowerCase().includes(q)) : files;
}
