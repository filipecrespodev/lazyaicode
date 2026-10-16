import { useState } from "react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { useBranchActions } from "./useBranchActions";
import { useT } from "../../../lib/i18n/react";

/** Creates a tag at `startSha` — opened from
 * the commit graph's right-click menu. */
export function CreateTagModal({
  repoId,
  startSha,
  onClose,
}: {
  repoId: string;
  startSha: string;
  onClose: () => void;
}) {
  const t = useT();
  const { createTagAt: create } = useBranchActions(repoId);
  const [name, setName] = useState("");

  return (
    <Modal onClose={onClose} align="top">
      <div className="flex flex-col text-sm">
        <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 px-3 py-2">
          <span className="font-semibold text-neutral-200">{t("gitBranches.createTag")}</span>
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            const trimmed = name.trim();
            if (!trimmed) return;
            create.mutate(
              { name: trimmed, sha: startSha },
              {
                onSuccess: () => {
                  toast.success(t("gitBranches.tagCreated", { name: trimmed }));
                  onClose();
                },
              },
            );
          }}
          className="space-y-3 p-4"
        >
          <p className="text-xs text-neutral-500">
            {t("gitBranches.fromCommit")} <span className="font-mono text-neutral-300">{startSha.slice(0, 7)}</span>
          </p>
          <label className="block text-xs text-neutral-500">
            {t("gitBranches.tagName")}
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.currentTarget.value)}
              required
              className="mt-0.5 w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
            />
          </label>
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-900"
            >
              {t("gitCommon.cancel")}
            </button>
            <button
              type="submit"
              disabled={create.isPending || !name.trim()}
              className="rounded bg-emerald-700 px-3 py-1.5 text-sm text-emerald-50 hover:bg-emerald-800 disabled:opacity-50"
            >
              {create.isPending ? t("gitCommon.creating") : t("gitBranches.createTag")}
            </button>
          </div>
        </form>
      </div>
    </Modal>
  );
}
