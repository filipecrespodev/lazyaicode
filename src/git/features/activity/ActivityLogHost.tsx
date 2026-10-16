import { Modal } from "@/components/Modal";
import { useTauriEvent } from "@/lib/tauri-events";
import { ActivityLogView } from "./ActivityLogView";
import { useActivityStore } from "./store";

type BackendActivity = { repoPath: string | null; level: "info" | "error"; message: string };

/** Collects what the backend reports and shows the log as a popup when it isn't pinned. */
export function ActivityLogHost() {
  const add = useActivityStore((s) => s.add);
  const open = useActivityStore((s) => s.open);
  const setOpen = useActivityStore((s) => s.setOpen);

  useTauriEvent<BackendActivity>("activity://log", (e) =>
    add({
      source: "backend",
      text: e.message,
      read: false,
      repoPath: e.repoPath,
      status: e.level === "error" ? "error" : "info",
    }),
  );

  if (!open) return null;
  return (
    <Modal onClose={() => setOpen(false)} width="w-[900px]">
      <div className="h-[70vh]">
        <ActivityLogView onClose={() => setOpen(false)} />
      </div>
    </Modal>
  );
}
