import { useSignCommits } from "./useSignCommits";
import { useT } from "../../../lib/i18n/react";
import { richText } from "@/lib/rich-text";

const CODES: Record<string, string> = {
  gpgsign: "commit.gpgsign = true",
  format: "gpg.format",
  key: "user.signingkey",
};

export function SigningSettings() {
  const t = useT();
  const { signCommits, setSignCommits } = useSignCommits();

  return (
    <div className="space-y-5">
      <div>
        <h2 className="mb-1 text-sm font-semibold text-neutral-200">{t("gitSigning.title")}</h2>
        <p className="mb-3 text-xs text-neutral-500">
          {richText(t("gitSigning.description"), (name) => (
            <code className="text-neutral-400">{CODES[name] ?? name}</code>
          ))}
        </p>
        <label className="flex items-start gap-2 text-sm text-neutral-300">
          <input
            type="checkbox"
            checked={signCommits}
            onChange={(e) => setSignCommits(e.currentTarget.checked)}
            className="mt-0.5 h-3.5 w-3.5 shrink-0 appearance-none rounded-sm border border-neutral-600 bg-neutral-900 checked:border-emerald-600 checked:bg-emerald-600"
          />
          <span>
            {t("gitSigning.signAll")}
            <span className="block text-xs text-neutral-500">{t("gitSigning.signAllHint")}</span>
          </span>
        </label>
      </div>
    </div>
  );
}
