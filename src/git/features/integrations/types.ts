export type HostingProvider = "gitlab" | "github";

/** A configured GitLab instance or GitHub account (github.com or Enterprise). */
export type HostingInstance = {
  id: string;
  name: string;
  baseUrl: string;
  token: string;
};

export type IntegrationsSettings = {
  gitlab: { instances: HostingInstance[] };
  github: { instances: HostingInstance[] };
};

/** The hosting service a repo's remote (origin first) points at, when one is configured. */
export type RemoteHosting = {
  provider: HostingProvider;
  instance: HostingInstance;
  /** `group/subgroup/repo` (GitLab) or `owner/repo` (GitHub). */
  projectPath: string;
};

export type GitLabProject = {
  id: number;
  defaultBranch: string | null;
};

export type MergeRequestInfo = {
  iid: number;
  title: string;
  sourceBranch: string;
  targetBranch: string;
  webUrl: string;
};

export type GitLabUser = {
  id: number;
  username: string;
  name: string;
};

export type GitLabPushData = {
  action: "pushed" | "created" | "removed";
  refType: "branch" | "tag";
  refName: string | null;
  commitCount: number;
  commitTo: string | null;
  commitTitle: string | null;
};

/** One entry of a project's activity feed. */
export type GitLabEvent = {
  id: number;
  /** e.g. "pushed to", "pushed new", "deleted", "opened", "accepted", "commented on". */
  actionName: string;
  /** ISO 8601. */
  createdAt: string;
  authorName: string;
  authorUsername: string;
  authorAvatarUrl: string | null;
  /** "MergeRequest", "Issue", "Note"... null for pushes. */
  targetType: string | null;
  targetTitle: string | null;
  targetIid: number | null;
  push: GitLabPushData | null;
};

export type CreateMergeRequestParams = {
  sourceBranch: string;
  targetBranch: string;
  title: string;
  description: string;
  assignee: GitLabUser | null;
  reviewers: GitLabUser[];
  /** GitLab only — GitHub sets these per repository. */
  removeSourceBranch: boolean;
  squash: boolean;
};
