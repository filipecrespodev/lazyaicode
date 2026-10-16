import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getConfig, setConfig } from "@/lib/config-store";
import { DEFAULT_AI_MODEL, isKnownModel } from "./models";
import type { AiSettings } from "./types";

const AI_SETTINGS_KEY = ["aiSettings"] as const;

const DEFAULT_SETTINGS: AiSettings = {
  commitMessageModel: DEFAULT_AI_MODEL,
  commitMessageCustomInstructions: "",
  composeCustomInstructions: "",
  conflictCustomInstructions: "",
};

export function useAiSettings() {
  const queryClient = useQueryClient();

  const { data } = useQuery({
    queryKey: AI_SETTINGS_KEY,
    queryFn: () => getConfig<AiSettings>("aiSettings", DEFAULT_SETTINGS),
  });

  // Merge defaults so settings saved before a field existed still have it;
  // um modelo de outro provedor (configuração antiga) volta para o padrão.
  const merged = { ...DEFAULT_SETTINGS, ...data };
  const settings: AiSettings = {
    commitMessageModel: isKnownModel(merged.commitMessageModel) ? merged.commitMessageModel : DEFAULT_AI_MODEL,
    commitMessageCustomInstructions: merged.commitMessageCustomInstructions,
    composeCustomInstructions: merged.composeCustomInstructions,
    conflictCustomInstructions: merged.conflictCustomInstructions,
  };

  // See useIntegrationsSettings.ts: update the cache synchronously so
  // controlled inputs track each keystroke, persist to disk in the background.
  const updateSettings = (patch: Partial<AiSettings>) => {
    const next = { ...settings, ...patch };
    queryClient.setQueryData(AI_SETTINGS_KEY, next);
    setConfig("aiSettings", next);
  };

  return {
    settingsLoaded: data !== undefined,
    settings,
    updateSettings,
  };
}
