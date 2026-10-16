import { Select } from "@/components/Select";
import { useLocalText } from "@/lib/use-local-text";
import { AI_MODELS } from "./models";
import { useAiSettings } from "./useAiSettings";
import { useT } from "../../../lib/i18n/react";

export function AiSettings() {
  const t = useT();
  const { settings, updateSettings } = useAiSettings();
  const [customInstructions, setCustomInstructions] = useLocalText(
    settings.commitMessageCustomInstructions,
    (v) => updateSettings({ commitMessageCustomInstructions: v }),
  );
  const [composeInstructions, setComposeInstructions] = useLocalText(
    settings.composeCustomInstructions,
    (v) => updateSettings({ composeCustomInstructions: v }),
  );

  const [conflictInstructions, setConflictInstructions] = useLocalText(
    settings.conflictCustomInstructions,
    (v) => updateSettings({ conflictCustomInstructions: v }),
  );

  return (
    <div className="space-y-5">
      <div className="space-y-2">
        <h2 className="text-sm font-semibold text-neutral-200">{t("gitAi.commitAi")}</h2>
        <p className="text-xs text-neutral-500">
          {t("gitAi.commitAiHelp")}
        </p>
        <div className="text-xs text-neutral-500">
          {t("gitAi.model")}
          <div className="mt-0.5">
            <Select
              value={settings.commitMessageModel}
              onChange={(v) => updateSettings({ commitMessageModel: v })}
              options={AI_MODELS.map((m) => ({ value: m.id, label: m.label }))}
            />
          </div>
        </div>
        <label className="block text-xs text-neutral-500">
          {t("gitAi.messageInstructions")}
          <textarea
            value={customInstructions}
            onChange={(e) => setCustomInstructions(e.currentTarget.value)}
            rows={4}
            placeholder={t("gitAi.messageInstructionsPlaceholder")}
            className="mt-0.5 w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
          />
        </label>
      </div>

      <div className="space-y-2">
        <h2 className="text-sm font-semibold text-neutral-200">{t("gitAi.compose")}</h2>
        <p className="text-xs text-neutral-500">
          {t("gitAi.composeHelp")}
        </p>
        <label className="block text-xs text-neutral-500">
          {t("gitAi.composeInstructions")}
          <textarea
            value={composeInstructions}
            onChange={(e) => setComposeInstructions(e.currentTarget.value)}
            rows={4}
            placeholder={t("gitAi.composeInstructionsPlaceholder")}
            className="mt-0.5 w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
          />
        </label>
      </div>

      <div className="space-y-2">
        <h2 className="text-sm font-semibold text-neutral-200">{t("gitAi.conflicts")}</h2>
        <p className="text-xs text-neutral-500">
          {t("gitAi.conflictsHelp")}
        </p>
        <label className="block text-xs text-neutral-500">
          {t("gitAi.conflictInstructions")}
          <textarea
            value={conflictInstructions}
            onChange={(e) => setConflictInstructions(e.currentTarget.value)}
            rows={3}
            placeholder={t("gitAi.conflictInstructionsPlaceholder")}
            className="mt-0.5 w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
          />
        </label>
      </div>
    </div>
  );
}
