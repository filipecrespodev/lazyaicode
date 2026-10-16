const MIN_PER_FILE = 500;

/** The files' patches as one text of at most `maxChars` (plus short markers):
 * every file gets an equal share first, so one huge file can't crowd out the rest. */
export function truncateDiff(files: { path: string; patch: string; isBinary?: boolean }[], maxChars: number): string {
  const text = files.filter((f) => !f.isBinary && f.patch);
  const perFile = Math.max(MIN_PER_FILE, Math.floor(maxChars / Math.max(text.length, 1)));
  let out = "";
  let omitted = 0;
  for (const f of text) {
    const body = f.patch.length > perFile ? `${f.patch.slice(0, perFile)}\n… (truncated)` : f.patch;
    const chunk = `--- ${f.path}\n${body}\n`;
    if (out.length + chunk.length > maxChars) omitted++;
    else out += chunk;
  }
  return omitted ? `${out}… ${omitted} more file(s) omitted\n` : out;
}
