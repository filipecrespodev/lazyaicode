import type { SelectOption } from "@/components/Select";
import { t } from "../../../lib/i18n/react";

/** Mirrors `SshKeySettings` in src-tauri/src/ssh_keys.rs (stored under `sshKeys`). */
export type SshKeySettings = {
  /** Private key path; null = OpenSSH's default identities (id_ed25519, id_rsa). */
  defaultKey: string | null;
  /** Workspace or repo directory → private key path. */
  overrides: Record<string, string>;
};

export type SshKeyInfo = { path: string; name: string; encrypted: boolean };

export const EMPTY_SSH_KEY_SETTINGS: SshKeySettings = { defaultKey: null, overrides: {} };

/** Select value meaning "no choice here": the default (Settings) or the next level up. */
export const INHERIT = "";

export function normalizeSshKeySettings(raw: Partial<SshKeySettings> | null | undefined): SshKeySettings {
  return { defaultKey: raw?.defaultKey ?? null, overrides: raw?.overrides ?? {} };
}

export function setOverride(settings: SshKeySettings, dir: string, key: string | null): SshKeySettings {
  const overrides = { ...settings.overrides };
  if (key) overrides[dir] = key;
  else delete overrides[dir];
  return { ...settings, overrides };
}

function isInside(path: string, dir: string): boolean {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  const [p, d] = [norm(path), norm(dir)];
  return p === d || p.startsWith(`${d}/`);
}

/** Same rule as the backend: the most specific override containing `path`, else the default. */
export function resolveKey(settings: SshKeySettings, path: string): string | null {
  const dir = Object.keys(settings.overrides)
    .filter((d) => isInside(path, d))
    .sort((a, b) => b.length - a.length)[0];
  return dir ? settings.overrides[dir] : settings.defaultKey;
}

/** Picker options; a configured key that's no longer on disk stays listed so it can be seen and changed. */
export function keyOptions(keys: SshKeyInfo[], selected: string | null, inheritLabel: string): SelectOption[] {
  const options = [
    { value: INHERIT, label: inheritLabel },
    ...keys.map((k) => ({ value: k.path, label: k.encrypted ? t("gitSsh.withPassphrase", { name: k.name }) : k.name })),
  ];
  if (selected && !keys.some((k) => k.path === selected)) {
    options.push({ value: selected, label: t("gitSsh.notFound", { path: selected }) });
  }
  return options;
}

export function keyName(path: string | null): string {
  return path ? (path.split(/[\\/]/).pop() ?? path) : t("gitSsh.openSshDefaults");
}
