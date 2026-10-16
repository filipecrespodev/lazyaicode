import { t } from "../../../lib/i18n/react";

export type IgnoreOption = { label: string; pattern: string };

/** Escapes glob metacharacters so a literal path matches only itself. A
 * trailing space would otherwise be trimmed by git. */
export function escapeIgnorePath(path: string): string {
  return path.replace(/[\\*?[\]]/g, "\\$&").replace(/ $/, "\\ ");
}

/**
 * The ways a status entry can be ignored, most specific first: the exact
 * file (anchored with a leading "/" so a same-named file elsewhere isn't
 * caught), every file with its extension, and each folder above it. An
 * untracked folder entry (`build/`) only offers folders.
 */
export function ignoreOptions(path: string): IgnoreOption[] {
  const isDir = path.endsWith("/");
  const parts = path.replace(/\/+$/, "").split("/").filter(Boolean);
  const name = parts[parts.length - 1] ?? "";
  const options: IgnoreOption[] = [];

  if (!isDir) {
    options.push({ label: t("gitWorking.ignoreFile", { name }), pattern: `/${escapeIgnorePath(parts.join("/"))}` });
    const dot = name.lastIndexOf(".");
    // ".env" has no extension, it's the whole name.
    if (dot > 0 && dot < name.length - 1) {
      const ext = escapeIgnorePath(name.slice(dot + 1));
      options.push({ label: t("gitWorking.ignoreExtension", { ext: name.slice(dot + 1) }), pattern: `*.${ext}` });
    }
  }
  for (let i = isDir ? parts.length : parts.length - 1; i > 0; i--) {
    const folder = parts.slice(0, i).join("/");
    options.push({ label: t("gitWorking.ignoreFolder", { folder }), pattern: `/${escapeIgnorePath(folder)}/` });
  }
  return options;
}
