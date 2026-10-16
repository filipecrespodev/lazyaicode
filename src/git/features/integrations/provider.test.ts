import { describe, expect, it } from "vitest";
import { gitHubTokenUrl, gitLabTokenUrl, matchInstance, parseRemoteUrl } from "./provider";

describe("parseRemoteUrl", () => {
  it("parses an scp-like SSH remote", () => {
    expect(parseRemoteUrl("git@gitlab.example.com:group/repo.git")).toEqual({
      host: "gitlab.example.com",
      path: "group/repo",
    });
  });

  it("parses an https remote, stripping the .git suffix", () => {
    expect(parseRemoteUrl("https://gitlab.example.com/group/repo.git")).toEqual({
      host: "gitlab.example.com",
      path: "group/repo",
    });
  });

  it("parses an ssh:// remote with an explicit port", () => {
    expect(parseRemoteUrl("ssh://git@gitlab.example.com:2222/group/repo.git")).toEqual({
      host: "gitlab.example.com",
      path: "group/repo",
    });
  });

  it("keeps nested subgroup paths intact", () => {
    expect(parseRemoteUrl("https://gitlab.example.com/group/subgroup/repo")).toEqual({
      host: "gitlab.example.com",
      path: "group/subgroup/repo",
    });
  });

  it("returns null for an unrecognizable remote", () => {
    expect(parseRemoteUrl("")).toBeNull();
    expect(parseRemoteUrl("not-a-remote")).toBeNull();
  });
});

describe("matchInstance", () => {
  const instances = [
    { id: "1", name: "Internal", baseUrl: "https://gitlab.example.com", token: "t1" },
    { id: "2", name: "Other", baseUrl: "https://git.other.com:8443", token: "t2" },
  ];

  it("matches by hostname regardless of remote URL form", () => {
    expect(matchInstance("git@gitlab.example.com:group/repo.git", instances)?.id).toBe("1");
    expect(matchInstance("https://git.other.com/group/repo.git", instances)?.id).toBe("2");
  });

  it("returns null when no configured instance matches", () => {
    expect(matchInstance("https://github.com/group/repo.git", instances)).toBeNull();
  });
});

describe("gitLabTokenUrl", () => {
  it("links to the prefilled token page on the instance", () => {
    expect(gitLabTokenUrl("https://gitlab.example.com/")).toBe(
      "https://gitlab.example.com/-/user_settings/personal_access_tokens?name=LazyAICode&scopes=api",
    );
  });
  it("returns null until the base URL is valid", () => {
    expect(gitLabTokenUrl("")).toBeNull();
    expect(gitLabTokenUrl("gitlab")).toBeNull();
  });
});

describe("gitHubTokenUrl", () => {
  it("links to the prefilled classic token page on github.com or Enterprise", () => {
    expect(gitHubTokenUrl("https://github.com")).toBe(
      "https://github.com/settings/tokens/new?description=LazyAICode&scopes=repo",
    );
    expect(gitHubTokenUrl("https://ghe.corp.com/")).toBe(
      "https://ghe.corp.com/settings/tokens/new?description=LazyAICode&scopes=repo",
    );
  });
});
