import { listen } from "@tauri-apps/api/event";
import { KeyRound } from "lucide-react";
import { useEffect, useState } from "react";
import { call } from "@/lib/tauri-client";
import { Modal } from "./Modal";
import { useT } from "../../lib/i18n/react";

/** Asks for an SSH key passphrase when the backend needs one (cached there for the session). */
export function SshPassphraseHost() {
  const t = useT();
  const [keyPath, setKeyPath] = useState<string | null>(null);
  const [value, setValue] = useState("");

  useEffect(() => {
    const show = (key: string) => {
      setValue("");
      setKeyPath(key);
    };
    const unlisten = listen<string>("ssh-passphrase-request", (e) => show(e.payload));
    // A request made before we were listening only shows up here.
    unlisten.then(() => call<string | null>("pending_ssh_passphrase")).then((key) => {
      if (key) show(key);
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  if (!keyPath) return null;

  const respond = (passphrase: string | null) => {
    call("submit_ssh_passphrase", { passphrase });
    setKeyPath(null);
  };

  return (
    <Modal onClose={() => respond(null)} width="w-[420px]">
      <form
        className="p-4"
        onSubmit={(e) => {
          e.preventDefault();
          respond(value);
        }}
      >
        <div className="mb-3 flex items-center gap-2">
          <KeyRound size={16} className="shrink-0 text-blue-500" />
          <h2 className="text-sm font-semibold text-neutral-100">{t("gitCommon.sshPassphraseTitle")}</h2>
        </div>
        <p className="mb-3 break-all text-sm text-neutral-300">{t("gitCommon.sshPassphrasePrompt", { key: keyPath })}</p>
        <input
          autoFocus
          type="password"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          className="mb-4 w-full rounded border border-neutral-700 bg-neutral-950 px-2 py-1.5 text-sm text-neutral-100 outline-none focus:border-neutral-500"
        />
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={() => respond(null)}
            className="rounded bg-neutral-800 px-3 py-1.5 text-sm text-neutral-200 hover:bg-neutral-700"
          >
            {t("gitCommon.cancel")}
          </button>
          <button type="submit" className="rounded bg-neutral-700 px-3 py-1.5 text-sm text-neutral-100 hover:bg-neutral-600">
            {t("gitCommon.unlock")}
          </button>
        </div>
      </form>
    </Modal>
  );
}
