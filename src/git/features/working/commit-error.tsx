import { toast } from "sonner";
import { errorMessage, hookFailure, hookOutputTail } from "@/lib/tauri-client";
import { t } from "../../../lib/i18n/react";

/** Toast for a failed commit; a hook failure shows the hook's output, which
 * is what the user needs to fix it (lint errors, a rejected message...). */
export function toastCommitError(error: unknown) {
  const failure = hookFailure(error);
  if (!failure) {
    toast.error(errorMessage(error));
    return;
  }
  toast.error(t("gitWorking.hookFailed", { hook: failure.hook }), {
    description: (
      <pre className="max-h-48 overflow-auto whitespace-pre-wrap font-mono text-[11px]">
        {hookOutputTail(failure.output) || t("gitWorking.noOutput")}
      </pre>
    ),
    duration: 15000,
  });
}
