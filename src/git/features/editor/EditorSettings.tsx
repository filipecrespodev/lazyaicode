import { Select } from "@/components/Select";
import { useDefaultEditor, useInstalledEditors } from "./useEditorSettings";
import { useT } from "../../../lib/i18n/react";

const SYSTEM = "__system__";

export function EditorSettings() {
  const t = useT();
  const { editorId, setEditorId } = useDefaultEditor();
  const { data: editors = [], isPending } = useInstalledEditors();
  const value = editors.some((e) => e.id === editorId) ? editorId! : SYSTEM;

  return (
    <div className="space-y-3">
      <div>
        <h2 className="mb-1 text-sm font-semibold text-neutral-200">{t("gitEditor.defaultEditor")}</h2>
        <p className="mb-3 text-xs text-neutral-500">
          {t("gitEditor.defaultEditorHint")}
        </p>
        <Select
          value={value}
          onChange={(v) => setEditorId(v === SYSTEM ? null : v)}
          options={[
            { value: SYSTEM, label: t("gitEditor.systemDefault") },
            ...editors.map((e) => ({ value: e.id, label: e.name })),
          ]}
        />
        {!isPending && editors.length === 0 && (
          <p className="mt-2 text-xs text-neutral-500">{t("gitEditor.noneFound")}</p>
        )}
      </div>
    </div>
  );
}
