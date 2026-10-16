import { t } from "../../../lib/i18n/react";

export type ActivityStatus = "running" | "ok" | "error" | "info";

export type ActivityEntry = {
  id: number;
  at: number;
  /** "command": an invoke from the UI; "backend": detail the backend reported (git CLI, network, SSH). */
  source: "command" | "backend";
  text: string;
  /** Read-only lookups (and polling) — hidden unless asked for. */
  read: boolean;
  /** Repo working directory, or null for app-wide activity. */
  repoPath: string | null;
  status: ActivityStatus;
  durationMs?: number;
  args?: unknown;
  error?: string;
};

/** Never logged: per-keystroke noise that may also carry typed secrets. */
const SKIPPED = new Set(["terminal_write", "terminal_resize", "pending_ssh_passphrase", "pending_https_credentials"]);
const READ = /^(get|list|scan)_|^(github|gitlab)_(get|list|search)_/;
const SECRET = /pass(word|phrase)?|token|secret|api_?key|authorization|credential/i;
const URL_PASSWORD = /([a-z][\w+.-]*:\/\/[^\s/@:]*):[^\s/@]+@/gi;
/** Comandos cujo argumento `path` é um repositório. */
const REPO_PATH_COMMANDS = new Set(["open_repository"]);

const MAX_STRING = 120;

export function isSkipped(cmd: string): boolean {
  return SKIPPED.has(cmd);
}

export function isRead(cmd: string): boolean {
  return READ.test(cmd);
}

/** Deep copy fit for display: secrets masked, long strings and lists cut. */
export function redact(value: unknown, depth = 0): unknown {
  if (typeof value === "string") {
    // URLs de remote podem trazer um token: https://usuario:<token>@host/...
    const masked = value.replace(URL_PASSWORD, "$1:•••@");
    return masked.length > MAX_STRING ? `${masked.slice(0, MAX_STRING)}… (${t("gitActivity.chars", { count: masked.length })})` : masked;
  }
  if (value === null || typeof value !== "object") return value;
  if (depth > 4) return "…";
  if (Array.isArray(value)) {
    const shown = value.slice(0, 20).map((v) => redact(v, depth + 1));
    return value.length > 20 ? [...shown, `… ${t("gitActivity.moreItems", { count: value.length - 20 })}`] : shown;
  }
  return Object.fromEntries(
    Object.entries(value).map(([k, v]) => [k, SECRET.test(k) && v != null ? "•••" : redact(v, depth + 1)]),
  );
}

function short(value: unknown): string {
  if (typeof value === "string") return value.length > 40 ? `${value.slice(0, 40)}…` : value;
  const json = JSON.stringify(value);
  return json.length > 40 ? `${json.slice(0, 40)}…` : json;
}

/** One-line form: `fetch_remote origin background=true`. Ids are left out (noise). */
export function commandLine(cmd: string, args: Record<string, unknown> | undefined): string {
  const parts = Object.entries((redact(args ?? {}) as Record<string, unknown>) ?? {})
    .filter(([k, v]) => k !== "repoId" && v !== undefined && v !== null && v !== false)
    .map(([k, v]) => (k === "remoteName" || k === "branch" ? short(v) : `${k}=${short(v)}`));
  return [cmd, ...parts].join(" ");
}

export function commandRepoPath(
  cmd: string,
  args: Record<string, unknown> | undefined,
  pathOfRepoId: (repoId: string) => string | undefined,
): string | null {
  if (typeof args?.repoId === "string") return pathOfRepoId(args.repoId) ?? null;
  if (REPO_PATH_COMMANDS.has(cmd) && typeof args?.path === "string") return args.path;
  return null;
}

/** Compares repo paths whatever their separators, trailing slash or drive-letter case. */
export function samePath(a: string | null, b: string | null): boolean {
  if (a === null || b === null) return false;
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").replace(/^[a-z]:/, (d) => d.toUpperCase());
  return norm(a) === norm(b);
}

export const LIMITS = { total: 1500, reads: 300 };

/** Appends, then drops the oldest reads past `limits.reads` and the oldest of anything past `limits.total`. */
export function appendEntry(entries: ActivityEntry[], entry: ActivityEntry, limits = LIMITS): ActivityEntry[] {
  let next = [...entries, entry];
  let reads = next.filter((e) => e.read).length;
  if (reads > limits.reads) {
    next = next.filter((e) => !(e.read && reads-- > limits.reads));
  }
  return next.length > limits.total ? next.slice(next.length - limits.total) : next;
}
