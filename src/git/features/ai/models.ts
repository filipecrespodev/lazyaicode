import type { AiModel } from "./types";
import { t } from "../../../lib/i18n/react";

/** Modelos do Claude Code (aliases do `--model`); "" usa o padrão da conta. */
export const AI_MODELS: AiModel[] = [
  { id: "haiku", get label() { return t("gitAi.modelHaiku"); } },
  { id: "sonnet", label: "Sonnet" },
  { id: "opus", label: "Opus" },
  { id: "", get label() { return t("gitAi.modelAccountDefault"); } },
];

export const DEFAULT_AI_MODEL = "haiku";

export const isKnownModel = (id: string) => AI_MODELS.some((m) => m.id === id);
