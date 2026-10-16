import { t } from "../../../lib/i18n/react";
import type { PullResult, PullStrategy, UpstreamStatus } from "./types";

const STRATEGY_KEYS: Record<PullStrategy, string> = {
  Merge: "gitRemote.strategyMerge",
  FfOnly: "gitRemote.strategyFfOnly",
  Rebase: "gitRemote.strategyRebase",
};

export const strategyLabel = (strategy: PullStrategy): string => t(STRATEGY_KEYS[strategy]);

/** Dropdown entries, the configured default first and marked as such. */
export function strategyOptions(configured: PullStrategy | undefined): { strategy: PullStrategy; label: string }[] {
  const all: PullStrategy[] = ["Merge", "FfOnly", "Rebase"];
  const ordered = configured ? [configured, ...all.filter((s) => s !== configured)] : all;
  return ordered.map((strategy) => ({
    strategy,
    label: strategy === configured ? t("gitRemote.strategyDefault", { label: strategyLabel(strategy) }) : strategyLabel(strategy),
  }));
}

/** Button tooltip: where the branch stands and what a click will do. */
export function pullTitle(status: UpstreamStatus | null | undefined): string {
  if (!status) return t("gitRemote.noUpstreamToPull");
  const counts =
    status.ahead === 0 && status.behind === 0
      ? t("gitRemote.upToDateSinceFetch", { upstream: status.upstream })
      : t("gitRemote.behindAhead", { behind: status.behind, ahead: status.ahead, upstream: status.upstream });
  return t("gitRemote.pullTitleHint", { counts, strategy: strategyLabel(status.strategy) });
}

const plural = (n: number) => t(n === 1 ? "gitRemote.commitsOne" : "gitRemote.commitsOther", { count: n });

/** Success toast text, or null when it stopped on conflicts (the conflict panel takes over). */
export function pullSummary(r: PullResult): string | null {
  switch (r.outcome) {
    case "UpToDate":
      return t("gitRemote.alreadyUpToDate", { upstream: r.upstream });
    case "FastForwarded":
      return t("gitRemote.fastForwarded", { commits: plural(r.commits), upstream: r.upstream });
    case "Merged":
      return t("gitRemote.merged", { commits: plural(r.commits), upstream: r.upstream });
    case "Rebased":
      return t(r.commits === 1 ? "gitRemote.rebasedOne" : "gitRemote.rebasedOther", {
        commits: plural(r.commits),
        upstream: r.upstream,
      });
    case "Conflicts":
      return null;
  }
}
