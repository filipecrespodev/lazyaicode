//! Drives a real `lazyaicode-agent` process over its stdio pipe — what the
//! Windows app does through `wsl.exe` — against a throwaway repo. Unix only:
//! it treats this machine's paths as the inside of a distro.
#![cfg(all(feature = "gui", unix))]

use lazyaicode_lib::git_client::git::log::LogQuery;
use lazyaicode_lib::git_client::state::AppState;
use lazyaicode_lib::git_client::wslpath::to_unc;
use serde_json::json;

fn init_repo(dir: &std::path::Path) {
    let repo = git2::Repository::init(dir).unwrap();
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("T", "t@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "first", &tree, &[]).unwrap();
}

#[tokio::test]
async fn repo_ops_round_trip_through_the_agent() {
    std::env::set_var("LAZYAICODE_AGENT_BIN", env!("CARGO_BIN_EXE_lazyaicode-agent"));
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    std::fs::create_dir(dir.join("repo")).unwrap();
    let dir = dir.join("repo");
    init_repo(&dir);

    let state = AppState::default();
    let unc = to_unc("Test", dir.to_str().unwrap());
    let (agent, linux) = state.route(&unc).await.unwrap().expect("a WSL path routes to an agent");
    assert_eq!(linux, dir.to_str().unwrap());
    assert_eq!(agent.distro, "Test");

    let summary = state.open_remote("Test", linux).await.unwrap();
    assert_eq!(summary.path, unc, "the app keeps knowing the repo by its UNC path");

    let actor = state.actor(&summary.repo_id).unwrap();
    // Typed results through RepoCommand/Reply.
    let page = actor.log(LogQuery { skip: 0, limit: 50, branch_filter: None }).await.unwrap();
    assert_eq!(page.commits.len(), 1);
    assert_eq!(page.commits[0].summary, "first");
    assert_eq!(actor.status().await.unwrap().dirty_files, 0);

    // A mutation, then the read that must observe it.
    std::fs::write(dir.join("b.txt"), "two\n").unwrap();
    actor.stage_path("b.txt".into()).await.unwrap();
    assert_eq!(actor.status().await.unwrap().staged_files, 1);

    // Errors keep their shape across the pipe.
    let err = actor.checkout_branch("nope".into()).await.unwrap_err();
    assert!(
        matches!(err, lazyaicode_lib::git_client::error::GitCommandError::Libgit2 { .. }),
        "{err:?}"
    );

    // Host ops: the workspace scan (what the sidebar lists) and the folder
    // picker come back as UNC paths.
    let parent_unc = to_unc("Test", dir.parent().unwrap().to_str().unwrap());
    let repos = state.scan_workspace(&parent_unc).await.unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].path, unc);
    assert_eq!(repos[0].name, "repo");
    let dirs: serde_json::Value = agent
        .call("list_dirs", json!({ "path": dir.parent().unwrap().to_str().unwrap() }))
        .await
        .unwrap();
    assert_eq!(dirs[0]["isRepo"], true);
    let fp: String = agent.call_as("fingerprint", json!({ "repoId": summary.repo_id })).await.unwrap();
    assert!(!fp.is_empty());
    assert!(!agent.home.is_empty() || std::env::var("HOME").is_err());
}

#[tokio::test]
async fn local_paths_do_not_route_to_an_agent() {
    let state = AppState::default();
    assert!(state.route("/home/me/repo").await.unwrap().is_none());
    assert!(state.route(r"C:\Users\me\repo").await.unwrap().is_none());
}
