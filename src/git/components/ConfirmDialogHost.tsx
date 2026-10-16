import { AlertTriangle, Info } from "lucide-react";
import { useConfirmStore } from "@/lib/confirm-store";
import { Modal } from "./Modal";
import { useT } from "../../lib/i18n/react";

export function ConfirmDialogHost() {
  const t = useT();
  const request = useConfirmStore((s) => s.request);
  const setRequest = useConfirmStore((s) => s.setRequest);

  if (!request) return null;

  const respond = (value: boolean) => {
    request.resolve(value);
    setRequest(null);
  };

  const isDestructive = request.kind === "warning" || request.kind === "error";

  return (
    <Modal onClose={() => respond(false)}>
      <div className="p-4">
        <div className="mb-3 flex items-center gap-2">
          {isDestructive ? (
            <AlertTriangle size={16} className="shrink-0 text-amber-200" />
          ) : (
            <Info size={16} className="shrink-0 text-blue-500" />
          )}
          <h2 className="text-sm font-semibold text-neutral-100">
            {request.title ?? t("gitCommon.confirm")}
          </h2>
        </div>
        <p className="mb-4 whitespace-pre-wrap text-sm text-neutral-300">{request.message}</p>
        <div className="flex justify-end gap-2">
          <button
            onClick={() => respond(false)}
            className="rounded bg-neutral-800 px-3 py-1.5 text-sm text-neutral-200 hover:bg-neutral-700"
          >
            {t("gitCommon.cancel")}
          </button>
          <button
            autoFocus
            onClick={() => respond(true)}
            className={`rounded px-3 py-1.5 text-sm text-neutral-100 ${
              isDestructive ? "bg-red-900 hover:brightness-110" : "bg-neutral-700 hover:bg-neutral-600"
            }`}
          >
            {t("gitCommon.confirm")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
