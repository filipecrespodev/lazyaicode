import { Select } from "@/components/Select";
import { AUTO_FETCH_CHOICES } from "./remotes";
import { useAutoFetchSettings } from "./useAutoFetch";
import { useT } from "../../../lib/i18n/react";

export function FetchSettings() {
  const { minutes, setMinutes } = useAutoFetchSettings();
  const t = useT();

  return (
    <div>
      <h2 className="mb-1 text-sm font-semibold text-neutral-200">{t("gitRemote.autoFetch")}</h2>
      <p className="mb-3 text-xs text-neutral-500">
        {t("gitRemote.autoFetchHint")}
      </p>
      <div className="w-56">
        <Select
          value={String(minutes)}
          onChange={(v) => setMinutes(Number(v))}
          options={AUTO_FETCH_CHOICES.map((m) => ({ value: String(m), label: m === 0 ? t("gitRemote.autoFetchOff") : t("gitRemote.autoFetchEvery", { minutes: m }) }))}
        />
      </div>
    </div>
  );
}
