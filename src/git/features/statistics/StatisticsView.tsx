import { useQuery } from "@tanstack/react-query";
import { GitCommitHorizontal, GitMerge, Loader2, MessageSquare, Trash2, X } from "lucide-react";
import { useEffect, useMemo } from "react";
import { listActivity } from "@/features/integrations/api";
import { HOSTING_LABELS } from "@/features/integrations/provider";
import type { GitLabEvent, RemoteHosting } from "@/features/integrations/types";
import { useHostingRemote } from "@/features/integrations/useHostingRemote";
import { formatDate, formatDateTime } from "@/lib/format-date";
import { gravatarUrl } from "@/lib/gravatar";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { buildHeatmap, relativeTime, type HeatmapDay } from "./activity";
import { getRepoStats } from "./api";
import type { RepoStats } from "./types";
import { overlayClass, overlayOrInline } from "@/embed";
import { intlLocale, t as translate, useT } from "../../../lib/i18n/react";

// Heatmap levels 0-4: empty cell, then the accent at increasing strength.
const LEVEL_CLASSES = [
  "bg-neutral-800",
  "bg-blue-500/25",
  "bg-blue-500/50",
  "bg-blue-500/75",
  "bg-blue-500",
] as const;

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-neutral-800 bg-neutral-900 px-4 py-3">
      <div className="text-xs text-neutral-500">{label}</div>
      <div className="mt-1 text-2xl font-semibold text-neutral-100">{value}</div>
    </div>
  );
}

function AuthorsTable({ authors }: { authors: RepoStats["authors"] }) {
  const t = useT();
  const max = authors[0]?.commits ?? 1;
  return (
    <section>
      <h3 className="mb-2 text-sm font-semibold text-neutral-200">{t("gitStats.authors")}</h3>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-neutral-800 text-left text-xs text-neutral-500">
            <th className="w-10 py-1.5 pl-2 font-normal">#</th>
            <th className="py-1.5 font-normal">{t("gitStats.author")}</th>
            <th className="w-56 py-1.5 font-normal">{t("gitStats.colCommits")}</th>
            <th className="w-28 py-1.5 pr-2 text-right font-normal">{t("gitStats.lastCommit")}</th>
          </tr>
        </thead>
        <tbody>
          {authors.map((a, i) => (
            <tr key={a.email || a.name} className="border-b border-neutral-800/60 hover:bg-neutral-900">
              <td className="py-2 pl-2 text-neutral-500">{i + 1}</td>
              <td className="min-w-0 py-2">
                <div className="flex items-center gap-2">
                  <img src={gravatarUrl(a.email, 40)} alt="" className="h-5 w-5 shrink-0 rounded-full" />
                  <span className="truncate text-neutral-200">{a.name}</span>
                  <span className="truncate text-xs text-neutral-500">&lt;{a.email}&gt;</span>
                </div>
              </td>
              <td className="py-2">
                <div className="flex items-center gap-2">
                  <span className="w-12 shrink-0 text-right tabular-nums text-neutral-200">{a.commits}</span>
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-neutral-800">
                    <div
                      className="h-full rounded-full bg-blue-500"
                      style={{ width: `${Math.max(2, (a.commits / max) * 100)}%` }}
                    />
                  </div>
                </div>
              </td>
              <td className="py-2 pr-2 text-right tabular-nums text-neutral-400">
                {formatDate(a.lastCommitTime)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function ActivityHeatmap({ commitTimes }: { commitTimes: number[] }) {
  const t = useT();
  const weeks = useMemo(() => buildHeatmap(commitTimes), [commitTimes]);
  const title = (d: HeatmapDay) =>
    t(d.count === 1 ? "gitStats.heatmapDayOne" : "gitStats.heatmapDayOther", { count: d.count, date: formatDate(d.date) });

  return (
    <section>
      <h3 className="mb-2 text-sm font-semibold text-neutral-200">{t("gitStats.heatmapTitle")}</h3>
      {/* One column per week, filled top to bottom (Sunday first). Cells
          shrink to fit the available width instead of scrolling; max-w caps
          them at ~14px on wide screens. */}
      <div
        className="grid max-w-[850px] grid-flow-col grid-rows-7 gap-[3px]"
        style={{ gridTemplateColumns: `repeat(${weeks.length}, minmax(0, 1fr))` }}
      >
        {weeks.flat().map((day) =>
          day.future ? (
            <div key={day.date.getTime()} />
          ) : (
            <div
              key={day.date.getTime()}
              title={title(day)}
              className={`aspect-square rounded-sm ${LEVEL_CLASSES[day.level]}`}
            />
          ),
        )}
      </div>
      <div className="mt-2 flex items-center gap-1 text-xs text-neutral-500">
        {t("gitStats.less")}
        {LEVEL_CLASSES.map((c) => (
          <span key={c} className={`h-3 w-3 rounded-sm ${c}`} />
        ))}
        {t("gitStats.more")}
      </div>
    </section>
  );
}

function targetLabel(hosting: RemoteHosting, targetType: string): { label: string; prefix: string } | undefined {
  if (targetType === "MergeRequest") {
    const labels = HOSTING_LABELS[hosting.provider];
    return { label: labels.request, prefix: labels.refPrefix };
  }
  if (targetType === "Issue") return { label: translate("gitStats.targetIssue"), prefix: "#" };
  if (targetType === "Milestone") return { label: translate("gitStats.targetMilestone"), prefix: "%" };
  return undefined;
}

/** Message keys for the action names the GitLab/GitHub feeds report (unknown ones pass through). */
const ACTION_KEYS: Record<string, string> = {
  opened: "actionOpened",
  closed: "actionClosed",
  reopened: "actionReopened",
  merged: "actionMerged",
  accepted: "actionAccepted",
  approved: "actionApproved",
  reviewed: "actionReviewed",
  updated: "actionUpdated",
  created: "actionCreated",
  deleted: "actionDeleted",
  destroyed: "actionDeleted",
  joined: "actionJoined",
  left: "actionLeft",
  "commented on": "actionCommentedOn",
  "created repository": "actionCreatedRepository",
  "published release": "actionPublishedRelease",
  starred: "actionStarred",
  forked: "actionForked",
  "added a member": "actionAddedMember",
};

/** The one-line description the hosting service's own activity feed shows for an event. */
function describeEvent(
  hosting: RemoteHosting,
  e: GitLabEvent,
): { icon: typeof GitCommitHorizontal; text: string; ref: string | null } {
  if (e.push) {
    const ref = e.push.refName;
    if (e.push.action === "removed") return { icon: Trash2, text: translate("gitStats.pushRemoved", { refType: e.push.refType }), ref };
    if (e.push.action === "created") return { icon: GitCommitHorizontal, text: translate("gitStats.pushCreated", { refType: e.push.refType }), ref };
    return { icon: GitCommitHorizontal, text: translate("gitStats.pushPushed", { refType: e.push.refType }), ref };
  }
  const target = e.targetType ? targetLabel(hosting, e.targetType) : undefined;
  const key = ACTION_KEYS[e.actionName];
  const translated = key ? translate(`gitStats.${key}`) : e.actionName;
  const action = translated.charAt(0).toUpperCase() + translated.slice(1);
  if (target) {
    const iid = e.targetIid !== null ? ` ${target.prefix}${e.targetIid}` : "";
    return { icon: GitMerge, text: `${action} ${target.label}${iid}`, ref: null };
  }
  if (e.targetType?.endsWith("Note")) return { icon: MessageSquare, text: action, ref: null };
  return { icon: GitCommitHorizontal, text: action, ref: null };
}

function ActivityFeed({ repoId, hosting }: { repoId: string; hosting: RemoteHosting }) {
  const t = useT();
  const { data: events, isPending, error } = useQuery({
    queryKey: queryKeys.gitlabEvents(repoId),
    queryFn: () => listActivity(hosting),
  });

  if (isPending) {
    return (
      <div className="flex items-center gap-2 text-sm text-neutral-500">
        <Loader2 size={14} className="animate-spin" /> {t("gitStats.loadingActivity")}
      </div>
    );
  }
  if (error) return <p className="text-sm text-red-400">{errorMessage(error)}</p>;
  if (!events?.length) return <p className="text-sm text-neutral-500">{t("gitStats.noActivity")}</p>;

  return (
    <ol className="relative">
      {/* Timeline rail behind the avatars. */}
      <div className="absolute top-2 bottom-2 left-[13px] w-px bg-neutral-800" />
      {events.map((e) => {
        const { icon: Icon, text, ref } = describeEvent(hosting, e);
        const createdAt = new Date(e.createdAt);
        const showCommit = e.push?.action === "pushed" && e.push.commitTo;
        const subject = e.push ? null : e.targetTitle;
        return (
          <li key={e.id} className="relative flex gap-3 py-2.5">
            {e.authorAvatarUrl ? (
              <img src={e.authorAvatarUrl} alt="" className="relative h-7 w-7 shrink-0 rounded-full" />
            ) : (
              <div className="relative flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-neutral-800 text-xs text-neutral-300">
                {e.authorName.charAt(0).toUpperCase()}
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline justify-between gap-2">
                <div className="min-w-0 truncate">
                  <span className="font-semibold text-neutral-200">{e.authorName}</span>{" "}
                  <span className="text-neutral-500">@{e.authorUsername}</span>
                </div>
                <span
                  title={formatDateTime(createdAt)}
                  className="shrink-0 text-xs text-neutral-500"
                >
                  {relativeTime(createdAt)}
                </span>
              </div>
              <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-neutral-400">
                <Icon size={13} className="shrink-0" />
                <span className="shrink-0">{text}</span>
                {ref && <span className="truncate font-mono text-xs text-blue-500">{ref}</span>}
              </div>
              {showCommit && (
                <div className="mt-1 truncate text-xs">
                  <span className="font-mono text-blue-500">{e.push!.commitTo!.slice(0, 8)}</span>
                  <span className="text-neutral-500"> · </span>
                  <span className="text-neutral-300">{e.push!.commitTitle}</span>
                  {e.push!.commitCount > 1 && (
                    <span className="text-neutral-500"> {t("gitStats.andMore", { count: e.push!.commitCount - 1 })}</span>
                  )}
                </div>
              )}
              {subject && <div className="mt-1 truncate text-xs text-neutral-300">{subject}</div>}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

/** Full-screen repository statistics: totals, per-author commits and a
 * year of commit activity for the checked-out branch, plus the project's
 * GitLab or GitHub activity feed when `origin` matches a configured instance. */
export function StatisticsView({ repoId, onClose }: { repoId: string; onClose?: () => void }) {
  const t = useT();
  const { data: stats, isPending, error } = useQuery({
    queryKey: queryKeys.repoStats(repoId),
    queryFn: () => getRepoStats(repoId),
    refetchOnMount: "always",
  });
  const hosting = useHostingRemote(repoId);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose?.();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return overlayOrInline(
    <div className={`${overlayClass(!!onClose)} flex flex-col bg-neutral-950 text-neutral-200`}>
      <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 px-3 py-2">
        <div className="flex items-baseline gap-3">
          <span className="text-sm font-semibold">{t("gitStats.title")}</span>
          <span className="text-xs text-neutral-500">{t("gitStats.subtitle")}</span>
        </div>
        {onClose && (
          <button
            title={t("gitStats.close")}
            onClick={onClose}
            className="rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
          >
            <X size={15} />
          </button>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <div
          className={`mx-auto grid gap-8 p-6 ${
            hosting ? "max-w-7xl xl:grid-cols-[minmax(0,1fr)_26rem]" : "max-w-5xl"
          }`}
        >
          <div className="min-w-0 space-y-6">
            {isPending && (
              <div className="flex items-center gap-2 text-sm text-neutral-500">
                <Loader2 size={14} className="animate-spin" /> {t("gitStats.loadingHistory")}
              </div>
            )}
            {error && <p className="text-sm text-red-400">{errorMessage(error)}</p>}
            {stats && (
              <>
                <div className="grid grid-cols-3 gap-3">
                  <StatCard label={t("gitStats.totalCommits")} value={stats.totalCommits.toLocaleString(intlLocale())} />
                  <StatCard label={t("gitStats.authors")} value={stats.authors.length.toLocaleString(intlLocale())} />
                  <StatCard
                    label={t("gitStats.firstCommit")}
                    value={stats.firstCommitTime !== null ? formatDate(stats.firstCommitTime) : "—"}
                  />
                </div>
                {stats.authors.length > 0 && <AuthorsTable authors={stats.authors} />}
                <ActivityHeatmap commitTimes={stats.recentCommitTimes} />
              </>
            )}
          </div>

          {hosting && (
            <section className="min-w-0">
              <h3 className="mb-2 text-sm font-semibold text-neutral-200">{t("gitStats.recentActivity")}</h3>
              <ActivityFeed repoId={repoId} hosting={hosting} />
            </section>
          )}
        </div>
      </div>
    </div>,
    !!onClose,
  );
}
