import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Plus, Trash2 } from "lucide-react";
import type { ReactNode } from "react";
import { useLocalText } from "@/lib/use-local-text";
import { tokenUrl } from "./provider";
import type { HostingInstance, HostingProvider } from "./types";
import { useIntegrationsSettings } from "./useIntegrationsSettings";
import { richText } from "@/lib/rich-text";
import { useT } from "../../../lib/i18n/react";

const PROVIDER_FORM = {
  gitlab: {
    namePlaceholderKey: "gitIntegrations.gitlabNamePlaceholder",
    urlPlaceholder: "https://gitlab.company.com",
    defaults: { name: "", baseUrl: "" },
  },
  github: {
    namePlaceholderKey: "gitIntegrations.githubNamePlaceholder",
    urlPlaceholder: "https://github.com",
    // Most people mean github.com; Enterprise users change the URL.
    defaults: { name: "GitHub", baseUrl: "https://github.com" },
  },
} as const;

/** Label row above a token input, with a direct link to where the token is created. */
export function TokenLabel({ url }: { url: string | null }) {
  const t = useT();
  return (
    <span className="flex items-center justify-between gap-2">
      {t("gitIntegrations.personalToken")}
      <button
        type="button"
        disabled={!url}
        onClick={() => url && openUrl(url)}
        title={url ?? t("gitIntegrations.setBaseUrlFirst")}
        className="flex items-center gap-1 text-blue-500 hover:underline disabled:cursor-not-allowed disabled:text-neutral-600 disabled:no-underline"
      >
        {t("gitIntegrations.generateToken")} <ExternalLink size={11} />
      </button>
    </span>
  );
}

function InstanceRow({
  provider,
  instance,
  onUpdate,
  onRemove,
}: {
  provider: HostingProvider;
  instance: HostingInstance;
  onUpdate: (patch: Partial<HostingInstance>) => void;
  onRemove: () => void;
}) {
  const t = useT();
  const [name, setName] = useLocalText(instance.name, (v) => onUpdate({ name: v }));
  const [baseUrl, setBaseUrl] = useLocalText(instance.baseUrl, (v) => onUpdate({ baseUrl: v }));
  const [token, setToken] = useLocalText(instance.token, (v) => onUpdate({ token: v }));
  const form = PROVIDER_FORM[provider];

  return (
    <div className="space-y-2 rounded border border-neutral-800 p-2">
      <div className="flex items-center gap-2">
        <input
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder={t(form.namePlaceholderKey)}
          className="min-w-0 flex-1 rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
        />
        <button
          onClick={onRemove}
          title={t("gitIntegrations.remove")}
          className="shrink-0 rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-red-400"
        >
          <Trash2 size={14} />
        </button>
      </div>
      <label className="block text-xs text-neutral-500">
        {t("gitIntegrations.baseUrl")}
        <input
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.currentTarget.value)}
          placeholder={form.urlPlaceholder}
          className="mt-0.5 w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
        />
      </label>
      <label className="block text-xs text-neutral-500">
        <TokenLabel url={tokenUrl(provider, baseUrl)} />
        <input
          type="password"
          value={token}
          onChange={(e) => setToken(e.currentTarget.value)}
          placeholder={t("gitIntegrations.notSet")}
          className="mt-0.5 w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
        />
      </label>
    </div>
  );
}

function ProviderSection({
  provider,
  title,
  description,
  addLabel,
  instances,
  onChange,
}: {
  provider: HostingProvider;
  title: string;
  description: ReactNode;
  addLabel: string;
  instances: HostingInstance[];
  onChange: (instances: HostingInstance[]) => void;
}) {
  const update = (id: string, patch: Partial<HostingInstance>) =>
    onChange(instances.map((i) => (i.id === id ? { ...i, ...patch } : i)));
  const add = () =>
    onChange([...instances, { id: crypto.randomUUID(), token: "", ...PROVIDER_FORM[provider].defaults }]);
  const remove = (id: string) => onChange(instances.filter((i) => i.id !== id));

  return (
    <details open className="rounded border border-neutral-800">
      <summary className="cursor-pointer select-none px-3 py-2 text-sm font-semibold text-neutral-200">
        {title}
      </summary>
      <div className="space-y-3 border-t border-neutral-800 p-3">
        <p className="text-xs text-neutral-500">{description}</p>
        {instances.map((instance) => (
          <InstanceRow
            key={instance.id}
            provider={provider}
            instance={instance}
            onUpdate={(patch) => update(instance.id, patch)}
            onRemove={() => remove(instance.id)}
          />
        ))}
        <button
          onClick={add}
          className="flex items-center gap-1.5 rounded border border-neutral-700 px-2 py-1 text-xs text-neutral-300 hover:bg-neutral-900"
        >
          <Plus size={12} /> {addLabel}
        </button>
      </div>
    </details>
  );
}

export function IntegrationsSettings() {
  const t = useT();
  const { settings, setGitLabInstances, setGitHubInstances } = useIntegrationsSettings();
  const code = (name: string) => <code>{name}</code>;

  return (
    <div className="space-y-3">
      <ProviderSection
        provider="gitlab"
        title="GitLab"
        description={richText(t("gitIntegrations.gitlabDescription"), code)}
        addLabel={t("gitIntegrations.addGitlab")}
        instances={settings.gitlab.instances}
        onChange={setGitLabInstances}
      />
      <ProviderSection
        provider="github"
        title="GitHub"
        description={richText(t("gitIntegrations.githubDescription"), code)}
        addLabel={t("gitIntegrations.addGithub")}
        instances={settings.github.instances}
        onChange={setGitHubInstances}
      />
    </div>
  );
}
