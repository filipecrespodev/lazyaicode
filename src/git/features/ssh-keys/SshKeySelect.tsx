import { Select } from "@/components/Select";
import { INHERIT, keyName, keyOptions, resolveKey, setOverride as withOverride } from "./settings";
import { useSshKeyList, useSshKeySettings } from "./useSshKeys";
import { useT } from "../../../lib/i18n/react";

/** Picks the SSH key for a workspace or repo directory; "inherit" falls back
 * to the enclosing workspace's, then the default from Settings. */
export function SshKeySelect({ dir }: { dir: string }) {
  const t = useT();
  const keys = useSshKeyList(dir);
  const { settings, setOverride } = useSshKeySettings();
  const selected = settings.overrides[dir] ?? null;
  const inherited = resolveKey(withOverride(settings, dir, null), dir);

  return (
    <Select
      value={selected ?? INHERIT}
      options={keyOptions(keys, selected, t("gitSsh.inherit", { name: keyName(inherited) }))}
      onChange={(v) => setOverride(dir, v || null)}
    />
  );
}
