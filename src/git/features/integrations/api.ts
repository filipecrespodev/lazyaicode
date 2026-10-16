import { call } from "@/lib/tauri-client";
import type {
  CreateMergeRequestParams,
  GitLabEvent,
  GitLabProject,
  GitLabUser,
  MergeRequestInfo,
  RemoteHosting,
} from "./types";

export const gitlabGetProject = (baseUrl: string, token: string, projectPath: string) =>
  call<GitLabProject>("gitlab_get_project", { baseUrl, token, projectPath });

export const gitlabListOpenMergeRequests = (baseUrl: string, token: string, projectPath: string) =>
  call<MergeRequestInfo[]>("gitlab_list_open_merge_requests", { baseUrl, token, projectPath });

export const gitlabCreateMergeRequest = (
  baseUrl: string,
  token: string,
  projectPath: string,
  params: CreateMergeRequestParams,
) =>
  call<MergeRequestInfo>("gitlab_create_merge_request", {
    baseUrl,
    token,
    projectPath,
    sourceBranch: params.sourceBranch,
    targetBranch: params.targetBranch,
    title: params.title,
    description: params.description,
    assigneeId: params.assignee?.id ?? null,
    reviewerIds: params.reviewers.map((r) => r.id),
    removeSourceBranch: params.removeSourceBranch,
    squash: params.squash,
  });

export const gitlabSearchMembers = (baseUrl: string, token: string, projectPath: string) =>
  call<GitLabUser[]>("gitlab_search_members", { baseUrl, token, projectPath });

export const gitlabListProjectEvents = (baseUrl: string, token: string, projectPath: string) =>
  call<GitLabEvent[]>("gitlab_list_project_events", { baseUrl, token, projectPath });

// GitHub — same result shapes as the GitLab calls above.

export const githubListOpenPullRequests = (baseUrl: string, token: string, projectPath: string) =>
  call<MergeRequestInfo[]>("github_list_open_pull_requests", { baseUrl, token, projectPath });

export const githubCreatePullRequest = (
  baseUrl: string,
  token: string,
  projectPath: string,
  params: CreateMergeRequestParams,
) =>
  call<MergeRequestInfo>("github_create_pull_request", {
    baseUrl,
    token,
    projectPath,
    sourceBranch: params.sourceBranch,
    targetBranch: params.targetBranch,
    title: params.title,
    description: params.description,
    assignee: params.assignee?.username ?? null,
    reviewers: params.reviewers.map((r) => r.username),
  });

export const githubListAssignees = (baseUrl: string, token: string, projectPath: string) =>
  call<GitLabUser[]>("github_list_assignees", { baseUrl, token, projectPath });

export const githubListRepoEvents = (baseUrl: string, token: string, projectPath: string) =>
  call<GitLabEvent[]>("github_list_repo_events", { baseUrl, token, projectPath });

// Provider-agnostic entry points: everything outside this folder goes through these.

const args = (h: RemoteHosting) => [h.instance.baseUrl, h.instance.token, h.projectPath] as const;

export const listOpenRequests = (h: RemoteHosting) =>
  h.provider === "github" ? githubListOpenPullRequests(...args(h)) : gitlabListOpenMergeRequests(...args(h));

export const createRequest = (h: RemoteHosting, params: CreateMergeRequestParams) =>
  h.provider === "github"
    ? githubCreatePullRequest(...args(h), params)
    : gitlabCreateMergeRequest(...args(h), params);

export const listMembers = (h: RemoteHosting) =>
  h.provider === "github" ? githubListAssignees(...args(h)) : gitlabSearchMembers(...args(h));

export const listActivity = (h: RemoteHosting) =>
  h.provider === "github" ? githubListRepoEvents(...args(h)) : gitlabListProjectEvents(...args(h));
