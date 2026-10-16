//! GERADO por `xtask/scripts/gen_git_commands.py` a partir de `git_cmds/*.rs`.
//! Não editar à mão: mude a função em `git_cmds` e rode o script de novo.

#![allow(unused_variables, clippy::too_many_lines)]

use lazyaicode_protocol::{ParamInfo, ParamType};
use serde::Deserialize;

use crate::git_cmds::*;
use crate::registry::{json_of, parse, Registry, Spec};

#[allow(clippy::too_many_lines)]
pub fn register(r: &mut Registry) {
    {
        #[allow(unused_imports)]
        use crate::git_cmds::ai_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            model: String,
            diff: String,
            custom_instructions: String,
        }
        r.add(
            Spec::new("ai.commit-message", "Ai generate commit message", "ai")
                .params(vec![
                    ParamInfo::req("model", ParamType::String),
                    ParamInfo::req("diff", ParamType::String),
                    ParamInfo::req("customInstructions", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        ai_cmds::ai_generate_commit_message(p.model, p.diff, p.custom_instructions, &e)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::ai_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            model: String,
            custom_instructions: String,
            answers: String,
        }
        r.add(
            Spec::new("ai.compose-commits", "Ai compose commits", "ai")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("model", ParamType::String),
                    ParamInfo::req("customInstructions", ParamType::String),
                    ParamInfo::req("answers", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        ai_cmds::ai_compose_commits(p.repo_id, p.model, p.custom_instructions, p.answers, &e.git(), &e)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::ai_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            tree_id: String,
            commits: Vec<ComposedCommitInput>,
            author_name: Option<String>,
            author_email: Option<String>,
            no_verify: Option<bool>,
        }
        r.add(
            Spec::new("git.working.apply-composed", "Apply composed commits", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("treeId", ParamType::String),
                    ParamInfo::req("commits", ParamType::Json),
                    ParamInfo::opt("authorName", ParamType::String),
                    ParamInfo::opt("authorEmail", ParamType::String),
                    ParamInfo::opt("noVerify", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        ai_cmds::apply_composed_commits(
                            p.repo_id,
                            p.tree_id,
                            p.commits,
                            p.author_name,
                            p.author_email,
                            p.no_verify,
                            &e.git(),
                        )
                        .await
                        .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::ai_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            model: String,
            path: String,
            ancestor: Option<String>,
            ours: String,
            theirs: String,
            custom_instructions: String,
        }
        r.add(
            Spec::new("ai.resolve-conflict", "Ai resolve conflict", "ai")
                .params(vec![
                    ParamInfo::req("model", ParamType::String),
                    ParamInfo::req("path", ParamType::String),
                    ParamInfo::opt("ancestor", ParamType::String),
                    ParamInfo::req("ours", ParamType::String),
                    ParamInfo::req("theirs", ParamType::String),
                    ParamInfo::req("customInstructions", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        ai_cmds::ai_resolve_conflict(p.model, p.path, p.ancestor, p.ours, p.theirs, p.custom_instructions, &e)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::ai_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            model: String,
            commits: Vec<String>,
            diff: String,
        }
        r.add(
            Spec::new("ai.pr-description", "Ai generate pr description", "ai")
                .params(vec![
                    ParamInfo::req("model", ParamType::String),
                    ParamInfo::req("commits", ParamType::Strings),
                    ParamInfo::req("diff", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(ai_cmds::ai_generate_pr_description(p.model, p.commits, p.diff, &e).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.branch.list", "List branches", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::list_branches(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.tag.list", "List tags", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::list_tags(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            start_point: Option<String>,
            checkout: bool,
        }
        r.add(
            Spec::new("git.branch.create", "Create branch", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::opt("startPoint", ParamType::String),
                    ParamInfo::req("checkout", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        branch_cmds::create_branch(p.repo_id, p.name, p.start_point, p.checkout, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
        }
        r.add(
            Spec::new("git.branch.checkout", "Checkout branch", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("name", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::checkout_branch(p.repo_id, p.name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            target_sha: String,
            remote_name: Option<String>,
        }
        r.add(
            Spec::new("git.branch.checkout-at", "Checkout at", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::req("targetSha", ParamType::String),
                    ParamInfo::opt("remoteName", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        branch_cmds::checkout_at(p.repo_id, p.name, p.target_sha, p.remote_name, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
        }
        r.add(
            Spec::new("git.branch.checkout-detached", "Checkout commit detached", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("sha", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::checkout_commit_detached(p.repo_id, p.sha, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
            create_commit: bool,
        }
        r.add(
            Spec::new("git.commit.cherry-pick", "Cherry pick", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("sha", ParamType::String),
                    ParamInfo::req("createCommit", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::cherry_pick(p.repo_id, p.sha, p.create_commit, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
            hard: bool,
        }
        r.add(
            Spec::new("git.reset.to-commit", "Reset to commit", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("sha", ParamType::String),
                    ParamInfo::req("hard", ParamType::Bool),
                ])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::reset_to_commit(p.repo_id, p.sha, p.hard, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            ref_name: String,
            hard: bool,
        }
        r.add(
            Spec::new("git.reset.to-ref", "Reset to ref", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("refName", ParamType::String),
                    ParamInfo::req("hard", ParamType::Bool),
                ])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::reset_to_ref(p.repo_id, p.ref_name, p.hard, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            target_sha: String,
        }
        r.add(
            Spec::new("git.branch.sync-to-remote", "Sync branch to remote", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("targetSha", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::sync_branch_to_remote(p.repo_id, p.target_sha, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
        }
        r.add(
            Spec::new("git.branch.delete-local", "Delete local branch", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("name", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::delete_local_branch(p.repo_id, p.name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: String,
            name: String,
        }
        r.add(
            Spec::new("git.branch.delete-remote", "Delete remote branch", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("remoteName", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                ])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        branch_cmds::delete_remote_branch(p.repo_id, p.remote_name, p.name, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_names: Vec<String>,
            name: String,
        }
        r.add(
            Spec::new("git.branch.delete-both", "Delete branch both", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("remoteNames", ParamType::Strings),
                    ParamInfo::req("name", ParamType::String),
                ])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        branch_cmds::delete_branch_both(p.repo_id, p.remote_names, p.name, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
        }
        r.add(
            Spec::new("git.branch.merge", "Merge branch", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("name", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::merge_branch(p.repo_id, p.name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            sha: String,
        }
        r.add(
            Spec::new("git.tag.create", "Create tag", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::req("sha", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::create_tag(p.repo_id, p.name, p.sha, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
        }
        r.add(
            Spec::new("git.tag.delete-local", "Delete local tag", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("name", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::delete_local_tag(p.repo_id, p.name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: String,
            name: String,
        }
        r.add(
            Spec::new("git.tag.push", "Push tag", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("remoteName", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::push_tag(p.repo_id, p.remote_name, p.name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: String,
            name: String,
        }
        r.add(
            Spec::new("git.tag.delete-remote", "Delete remote tag", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("remoteName", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                ])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        branch_cmds::delete_remote_tag(p.repo_id, p.remote_name, p.name, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::branch_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: String,
        }
        r.add(
            Spec::new("git.tag.list-remote", "List remote tags", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("remoteName", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(branch_cmds::list_remote_tags(p.repo_id, p.remote_name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::config_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {}
        r.add(Spec::new("git.config.identity", "Get global git identity", "git").params(vec![]).doc("Reads the machine's global git identity (`user.name`/`user.email`) — used to seed the \"Default\" commit profile the first time the app runs.").read_only().run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(config_cmds::get_global_git_identity().await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::config_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {}
        r.add(
            Spec::new("git.config.editors", "List installed editors", "git")
                .params(vec![])
                .doc("Editors found on this machine, for the default-editor setting.")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(config_cmds::list_installed_editors().await)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::config_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {}
        r.add(Spec::new("system.fonts", "List system fonts", "system").params(vec![]).doc("Font families installed on this machine, for the Appearance font pickers: `fc-list` on Linux/macOS, WPF's font list through PowerShell on Windows.").read_only().run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(config_cmds::list_system_fonts().await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::config_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            rel_path: String,
        }
        r.add(
            Spec::new("git.config.repo-file-path", "Repo file path", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("relPath", ParamType::String)])
                .doc("Absolute path of `rel_path` inside the repo's working directory.")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(config_cmds::repo_file_path(p.repo_id, p.rel_path, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::config_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            command: String,
            path: String,
        }
        r.add(
            Spec::new("git.config.open-in-editor", "Open in editor", "git")
                .params(vec![ParamInfo::req("command", ParamType::String), ParamInfo::req("path", ParamType::String)])
                .doc("Opens `path` with the editor launcher `command` (from `list_installed_editors`).")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(config_cmds::open_in_editor(p.command, p.path).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::diff_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
        }
        r.add(
            Spec::new("git.diff.commit", "Get commit diff", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("sha", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(diff_cmds::get_commit_diff(p.repo_id, p.sha, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::diff_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            from_sha: String,
            to_sha: String,
        }
        r.add(
            Spec::new("git.diff.range", "Get range diff", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("fromSha", ParamType::String),
                    ParamInfo::req("toSha", ParamType::String),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(diff_cmds::get_range_diff(p.repo_id, p.from_sha, p.to_sha, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::diff_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: Option<String>,
            path: String,
        }
        r.add(
            Spec::new("git.diff.file-content", "Get file content", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::opt("sha", ParamType::String),
                    ParamInfo::req("path", ParamType::String),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(diff_cmds::get_file_content(p.repo_id, p.sha, p.path, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::github_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("github.pr.list", "Github list open pull requests", "github")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        github_cmds::github_list_open_pull_requests(p.base_url, p.token, p.project_path)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::github_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
            source_branch: String,
            target_branch: String,
            title: String,
            description: String,
            assignee: Option<String>,
            reviewers: Vec<String>,
        }
        r.add(
            Spec::new("github.pr.create", "Github create pull request", "github")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                    ParamInfo::req("sourceBranch", ParamType::String),
                    ParamInfo::req("targetBranch", ParamType::String),
                    ParamInfo::req("title", ParamType::String),
                    ParamInfo::req("description", ParamType::String),
                    ParamInfo::opt("assignee", ParamType::String),
                    ParamInfo::req("reviewers", ParamType::Strings),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        github_cmds::github_create_pull_request(
                            p.base_url,
                            p.token,
                            p.project_path,
                            p.source_branch,
                            p.target_branch,
                            p.title,
                            p.description,
                            p.assignee,
                            p.reviewers,
                        )
                        .await
                        .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::github_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("github.assignees", "Github list assignees", "github")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        github_cmds::github_list_assignees(p.base_url, p.token, p.project_path).await.map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::github_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("github.events", "Github list repo events", "github")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        github_cmds::github_list_repo_events(p.base_url, p.token, p.project_path)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::gitlab_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("gitlab.project", "Gitlab get project", "gitlab")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(gitlab_cmds::gitlab_get_project(p.base_url, p.token, p.project_path).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::gitlab_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("gitlab.mr.list", "Gitlab list open merge requests", "gitlab")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        gitlab_cmds::gitlab_list_open_merge_requests(p.base_url, p.token, p.project_path)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::gitlab_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
            source_branch: String,
            target_branch: String,
            title: String,
            description: String,
            assignee_id: Option<u64>,
            reviewer_ids: Vec<u64>,
            remove_source_branch: bool,
            squash: bool,
        }
        r.add(
            Spec::new("gitlab.mr.create", "Gitlab create merge request", "gitlab")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                    ParamInfo::req("sourceBranch", ParamType::String),
                    ParamInfo::req("targetBranch", ParamType::String),
                    ParamInfo::req("title", ParamType::String),
                    ParamInfo::req("description", ParamType::String),
                    ParamInfo::opt("assigneeId", ParamType::Int),
                    ParamInfo::req("reviewerIds", ParamType::Json),
                    ParamInfo::req("removeSourceBranch", ParamType::Bool),
                    ParamInfo::req("squash", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        gitlab_cmds::gitlab_create_merge_request(
                            p.base_url,
                            p.token,
                            p.project_path,
                            p.source_branch,
                            p.target_branch,
                            p.title,
                            p.description,
                            p.assignee_id,
                            p.reviewer_ids,
                            p.remove_source_branch,
                            p.squash,
                        )
                        .await
                        .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::gitlab_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("gitlab.members", "Gitlab search members", "gitlab")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        gitlab_cmds::gitlab_search_members(p.base_url, p.token, p.project_path).await.map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::gitlab_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            base_url: String,
            token: String,
            project_path: String,
        }
        r.add(
            Spec::new("gitlab.events", "Gitlab list project events", "gitlab")
                .params(vec![
                    ParamInfo::req("baseUrl", ParamType::String),
                    ParamInfo::req("token", ParamType::String),
                    ParamInfo::req("projectPath", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        gitlab_cmds::gitlab_list_project_events(p.base_url, p.token, p.project_path)
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::history_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
            mainline: Option<u32>,
        }
        r.add(
            Spec::new("git.commit.revert", "Revert commit", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("sha", ParamType::String),
                    ParamInfo::opt("mainline", ParamType::Int),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(history_cmds::revert_commit(p.repo_id, p.sha, p.mainline, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::history_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            query: FileHistoryQuery,
        }
        r.add(
            Spec::new("git.history.file", "Get file history", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("query", ParamType::Json)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(history_cmds::get_file_history(p.repo_id, p.query, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::history_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
            path: String,
            old_path: Option<String>,
        }
        r.add(
            Spec::new("git.history.commit-file-diff", "Get commit file diff", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("sha", ParamType::String),
                    ParamInfo::req("path", ParamType::String),
                    ParamInfo::opt("oldPath", ParamType::String),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        history_cmds::get_commit_file_diff(p.repo_id, p.sha, p.path, p.old_path, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::history_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            path: String,
            sha: Option<String>,
            start_line: u32,
            max_lines: u32,
        }
        r.add(
            Spec::new("git.history.blame", "Get blame", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("path", ParamType::String),
                    ParamInfo::opt("sha", ParamType::String),
                    ParamInfo::req("startLine", ParamType::Int),
                    ParamInfo::req("maxLines", ParamType::Int),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        history_cmds::get_blame(p.repo_id, p.path, p.sha, p.start_line, p.max_lines, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::history_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            a: String,
            b: String,
            limit: u32,
        }
        r.add(
            Spec::new("git.history.compare", "Get branch compare", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("a", ParamType::String),
                    ParamInfo::req("b", ParamType::String),
                    ParamInfo::req("limit", ParamType::Int),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        history_cmds::get_branch_compare(p.repo_id, p.a, p.b, p.limit, &e.git()).await.map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::history_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            from_sha: String,
            to_sha: String,
        }
        r.add(
            Spec::new("git.history.tree-diff", "Get tree diff", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("fromSha", ParamType::String),
                    ParamInfo::req("toSha", ParamType::String),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(history_cmds::get_tree_diff(p.repo_id, p.from_sha, p.to_sha, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::interactive_rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            from_sha: String,
        }
        r.add(
            Spec::new("git.irebase.plan", "Get rebase plan", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("fromSha", ParamType::String)])
                .doc("Commits an interactive rebase starting at `from_sha` would rewrite.")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        interactive_rebase_cmds::get_rebase_plan(p.repo_id, p.from_sha, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::interactive_rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            upstream: Option<String>,
            items: Vec<TodoItem>,
        }
        r.add(
            Spec::new("git.irebase.start", "Start interactive rebase", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::opt("upstream", ParamType::String),
                    ParamInfo::req("items", ParamType::Json),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        interactive_rebase_cmds::start_interactive_rebase(p.repo_id, p.upstream, p.items, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            query: LogQuery,
        }
        r.add(
            Spec::new("git.log.list", "Get commit log", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("query", ParamType::Json)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(log_cmds::get_commit_log(p.repo_id, p.query, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.log.head", "Get head commit", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(log_cmds::get_head_commit(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
        }
        r.add(Spec::new("git.log.signature", "Get commit signature", "git").params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("sha", ParamType::String)]).doc("Whether a commit is signed (and how), for the commit details panel. Asked for the selected commit only, never per graph row.").read_only().run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(log_cmds::get_commit_signature(p.repo_id, p.sha, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.log.stats", "Get repo stats", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(log_cmds::get_repo_stats(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            ref_name: String,
        }
        r.add(
            Spec::new("git.reflog.list", "Get reflog", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("refName", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(log_cmds::get_reflog(p.repo_id, p.ref_name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            query: String,
            mode: SearchMode,
        }
        r.add(
            Spec::new("git.log.search", "Search commits", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("query", ParamType::String),
                    ParamInfo::req("mode", ParamType::Json),
                ])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(log_cmds::search_commits(p.repo_id, p.query, p.mode, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::log_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(Spec::new("git.repo.fingerprint", "Get repo fingerprint", "git").params(vec![ParamInfo::req("repoId", ParamType::String)]).doc("Token that changes when refs/HEAD/stash move (see `git::fingerprint`). Read straight from disk, not through the repo actor: it must answer even while the actor is busy.").read_only().run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(log_cmds::get_repo_fingerprint(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::pull_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            strategy: Option<PullStrategy>,
        }
        r.add(
            Spec::new("git.pull.pull", "Pull branch", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::opt("strategy", ParamType::Json)])
                .doc("`strategy`: overrides the repo's pull config for this one pull.")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(pull_cmds::pull_branch(p.repo_id, p.strategy, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::pull_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.pull.upstream-status", "Get upstream status", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(pull_cmds::get_upstream_status(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            onto_branch: String,
        }
        r.add(
            Spec::new("git.rebase.start", "Start rebase", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("ontoBranch", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(rebase_cmds::start_rebase(p.repo_id, p.onto_branch, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.rebase.status", "Get rebase status", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(rebase_cmds::get_rebase_status(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            path: String,
            resolution: ConflictResolution,
        }
        r.add(
            Spec::new("git.rebase.resolve-conflict", "Resolve rebase conflict", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("path", ParamType::String),
                    ParamInfo::req("resolution", ParamType::Json),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        rebase_cmds::resolve_rebase_conflict(p.repo_id, p.path, p.resolution, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.rebase.continue", "Continue rebase", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(rebase_cmds::continue_rebase(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::rebase_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.rebase.abort", "Abort rebase", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(rebase_cmds::abort_rebase(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: Option<String>,
            background: Option<bool>,
        }
        r.add(Spec::new("git.remote.fetch", "Fetch remote", "git").params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::opt("remoteName", ParamType::String), ParamInfo::opt("background", ParamType::Bool)]).doc("`remote_name`: `None` fetches every remote. `background`: a fetch the user didn't ask for (on open, periodic), which must not prompt for an SSH key passphrase or HTTPS credentials.").run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(remote_cmds::fetch_remote(p.repo_id, p.remote_name, p.background, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: String,
        }
        r.add(
            Spec::new("git.remote.url", "Get remote url", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("remoteName", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(remote_cmds::get_remote_url(p.repo_id, p.remote_name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            remote_name: Option<String>,
            branch: String,
            force: bool,
        }
        r.add(Spec::new("git.remote.push", "Push branch", "git").params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::opt("remoteName", ParamType::String), ParamInfo::req("branch", ParamType::String), ParamInfo::req("force", ParamType::Bool)]).doc("`remote_name`: `None` pushes where `git push` would (upstream, else `origin`, else the only remote), failing with `RemoteRequired` when that's ambiguous so the UI can ask.").run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(remote_cmds::push_branch(p.repo_id, p.remote_name, p.branch, p.force, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.remote.list", "List remotes", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(remote_cmds::list_remotes(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            url: String,
        }
        r.add(
            Spec::new("git.remote.add", "Add remote", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::req("url", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(remote_cmds::add_remote(p.repo_id, p.name, p.url, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
        }
        r.add(
            Spec::new("git.remote.remove", "Remove remote", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("name", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(remote_cmds::remove_remote(p.repo_id, p.name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            new_name: String,
        }
        r.add(
            Spec::new("git.remote.rename", "Rename remote", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::req("newName", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(remote_cmds::rename_remote(p.repo_id, p.name, p.new_name, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::remote_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            url: String,
        }
        r.add(
            Spec::new("git.remote.set-url", "Set remote url", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::req("url", ParamType::String),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(remote_cmds::set_remote_url(p.repo_id, p.name, p.url, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            path: String,
        }
        r.add(Spec::new("git.repo.open", "Open repository", "git").params(vec![ParamInfo::req("path", ParamType::String)]).doc("A `\\\\wsl.localhost\\<distro>\\...` path opens through that distro's agent (see `git_client::wsl`); anything else in this process.").run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(repo_cmds::open_repository(p.path, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.repo.status", "Get repo status", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_cmds::get_repo_status(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.repo.close", "Close repository", "git").params(vec![ParamInfo::req("repoId", ParamType::String)]).doc("").run(
                |e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_cmds::close_repository(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                },
            ),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.worktree.list", "List worktrees", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::list_worktrees(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            args: AddWorktreeArgs,
        }
        r.add(
            Spec::new("git.worktree.add", "Add worktree", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("args", ParamType::Json)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::add_worktree(p.repo_id, p.args, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            name: String,
            force: bool,
        }
        r.add(
            Spec::new("git.worktree.remove", "Remove worktree", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("name", ParamType::String),
                    ParamInfo::req("force", ParamType::Bool),
                ])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::remove_worktree(p.repo_id, p.name, p.force, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.worktree.prune", "Prune worktrees", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::prune_worktrees(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.submodule.list", "List submodules", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::list_submodules(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(Spec::new("git.submodule.count", "Get submodule count", "git").params(vec![ParamInfo::req("repoId", ParamType::String)]).doc("Decides whether the repo menu offers Submodules at all; cheap enough to ask on every repo open, unlike `list_submodules`.").read_only().run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(repo_tools_cmds::get_submodule_count(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            paths: Vec<String>,
        }
        r.add(
            Spec::new("git.submodule.update", "Update submodules", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("paths", ParamType::Strings)])
                .doc("`git submodule update --init` for `paths` (every submodule when empty).")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::update_submodules(p.repo_id, p.paths, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.lfs.status", "Get lfs status", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("Off the actor: the first call may spawn `git lfs version`.")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(repo_tools_cmds::get_lfs_status(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::repo_tools_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(Spec::new("git.lfs.pull", "Run lfs pull", "git").params(vec![ParamInfo::req("repoId", ParamType::String)]).doc("Runs off the actor so a long download doesn't hold up every other command for this repo; git-lfs does its own locking.").run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(repo_tools_cmds::run_lfs_pull(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::stash_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            message: Option<String>,
            include_untracked: bool,
        }
        r.add(
            Spec::new("git.stash.save", "Stash save", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::opt("message", ParamType::String),
                    ParamInfo::req("includeUntracked", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        stash_cmds::stash_save(p.repo_id, p.message, p.include_untracked, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::stash_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(Spec::new("git.stash.list", "Stash list", "git").params(vec![ParamInfo::req("repoId", ParamType::String)]).doc("").run(
            |e, v| async move {
                let p: P = parse(v)?;
                json_of(stash_cmds::stash_list(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
            },
        ));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::stash_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            index: usize,
        }
        r.add(
            Spec::new("git.stash.pop", "Stash pop", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("index", ParamType::Int)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(stash_cmds::stash_pop(p.repo_id, p.index, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::stash_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            index: usize,
        }
        r.add(
            Spec::new("git.stash.apply", "Stash apply", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("index", ParamType::Int)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(stash_cmds::stash_apply(p.repo_id, p.index, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::stash_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            index: usize,
        }
        r.add(
            Spec::new("git.stash.drop", "Stash drop", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("index", ParamType::Int)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(stash_cmds::stash_drop(p.repo_id, p.index, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::undo_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.undo.undo", "Undo last action", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("Reverts the last local action; returns its label.")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(undo_cmds::undo_last_action(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::undo_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.undo.redo", "Redo last action", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("Re-applies the last undone action; returns its label.")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(undo_cmds::redo_last_action(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::undo_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.undo.status", "Get undo status", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(undo_cmds::get_undo_status(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.working.status", "Get working status", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::get_working_status(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            path: String,
        }
        r.add(
            Spec::new("git.working.stage-path", "Stage path", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("path", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::stage_path(p.repo_id, p.path, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.working.stage-all", "Stage all", "git").params(vec![ParamInfo::req("repoId", ParamType::String)]).doc("").run(
                |e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::stage_all(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                },
            ),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            path: String,
        }
        r.add(
            Spec::new("git.working.unstage-path", "Unstage path", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("path", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::unstage_path(p.repo_id, p.path, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.working.unstage-all", "Unstage all", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::unstage_all(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            path: String,
        }
        r.add(
            Spec::new("git.working.discard-path", "Discard path", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("path", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::discard_path(p.repo_id, p.path, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
        }
        r.add(
            Spec::new("git.working.discard-all", "Discard all", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String)])
                .doc("")
                .destructive()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::discard_all(p.repo_id, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            path: String,
            action: LineAction,
            selection: LineSelection,
        }
        r.add(
            Spec::new("git.working.apply-lines", "Apply lines", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("path", ParamType::String),
                    ParamInfo::req("action", ParamType::Json),
                    ParamInfo::req("selection", ParamType::Json),
                ])
                .doc("Stages, unstages or discards only the selected changed lines of `path` (a hunk is just all of its lines).")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        working_cmds::apply_lines(p.repo_id, p.path, p.action, p.selection, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            pattern: String,
            local: bool,
        }
        r.add(Spec::new("git.working.ignore", "Add ignore pattern", "git").params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("pattern", ParamType::String), ParamInfo::req("local", ParamType::Bool)]).doc("Appends `pattern` to `.gitignore`, or to `.git/info/exclude` when `local`. Resolves to `false` when the pattern was already there.").run(|e, v| async move {
            let p: P = parse(v)?;
            json_of(working_cmds::add_ignore_pattern(p.repo_id, p.pattern, p.local, &e.git()).await.map_err(crate::EngineError::from)?)
        }));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            staged: bool,
        }
        r.add(
            Spec::new("git.working.diff", "Get workdir diff", "git")
                .params(vec![ParamInfo::req("repoId", ParamType::String), ParamInfo::req("staged", ParamType::Bool)])
                .doc("")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(working_cmds::get_workdir_diff(p.repo_id, p.staged, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            summary: String,
            body: String,
            author_name: Option<String>,
            author_email: Option<String>,
            amend: bool,
            no_verify: Option<bool>,
        }
        r.add(
            Spec::new("git.working.commit", "Git commit changes", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("summary", ParamType::String),
                    ParamInfo::req("body", ParamType::String),
                    ParamInfo::opt("authorName", ParamType::String),
                    ParamInfo::opt("authorEmail", ParamType::String),
                    ParamInfo::req("amend", ParamType::Bool),
                    ParamInfo::opt("noVerify", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        working_cmds::git_commit_changes(
                            p.repo_id,
                            p.summary,
                            p.body,
                            p.author_name,
                            p.author_email,
                            p.amend,
                            p.no_verify,
                            &e.git(),
                        )
                        .await
                        .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::working_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            repo_id: String,
            sha: String,
            summary: String,
            body: String,
            no_verify: Option<bool>,
        }
        r.add(
            Spec::new("git.working.amend-message", "Amend commit message", "git")
                .params(vec![
                    ParamInfo::req("repoId", ParamType::String),
                    ParamInfo::req("sha", ParamType::String),
                    ParamInfo::req("summary", ParamType::String),
                    ParamInfo::req("body", ParamType::String),
                    ParamInfo::opt("noVerify", ParamType::Bool),
                ])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        working_cmds::amend_commit_message(p.repo_id, p.sha, p.summary, p.body, p.no_verify, &e.git())
                            .await
                            .map_err(crate::EngineError::from)?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {}
        r.add(
            Spec::new("wsl.distros", "List wsl distros", "wsl")
                .params(vec![])
                .doc("Installed WSL distros (empty off Windows).")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(wsl_cmds::list_wsl_distros().await)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            distro: String,
        }
        r.add(
            Spec::new("wsl.home", "Wsl home", "wsl")
                .params(vec![ParamInfo::req("distro", ParamType::String)])
                .doc("The distro's home folder, as a UNC path: where the WSL folder picker starts.")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(wsl_cmds::wsl_home(p.distro, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            path: String,
        }
        r.add(
            Spec::new("wsl.dirs", "List wsl dirs", "wsl")
                .params(vec![ParamInfo::req("path", ParamType::String)])
                .doc("Subfolders of a WSL UNC path, for the in-app folder picker (the native dialog crawls over 9P).")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(wsl_cmds::list_wsl_dirs(p.path, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            distro: Option<String>,
        }
        r.add(
            Spec::new("git.ssh.list-keys", "List ssh keys", "git")
                .params(vec![ParamInfo::opt("distro", ParamType::String)])
                .doc("Keys in `~/.ssh` of this machine, or of `distro` when given.")
                .read_only()
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(wsl_cmds::list_ssh_keys(p.distro, &e.git()).await.map_err(crate::EngineError::from)?)
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            passphrase: Option<String>,
        }
        r.add(
            Spec::new("git.ssh.submit-passphrase", "Submit ssh passphrase", "git")
                .params(vec![ParamInfo::opt("passphrase", ParamType::String)])
                .doc("Answers the passphrase prompt, wherever it came from (this process or an agent: the one that asked is not tracked).")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        wsl_cmds::submit_ssh_passphrase(p.passphrase, &e.git())
                            .await
                            .map_err(|()| crate::EngineError::Failed("submit_ssh_passphrase falhou".into()))?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {}
        r.add(Spec::new("git.ssh.pending-passphrase", "Pending ssh passphrase", "git").params(vec![]).doc("").read_only().run(
            |e, v| async move {
                let p: P = parse(v)?;
                json_of(
                    wsl_cmds::pending_ssh_passphrase(&e.git())
                        .await
                        .map_err(|()| crate::EngineError::Failed("pending_ssh_passphrase falhou".into()))?,
                )
            },
        ));
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {
            username: Option<String>,
            password: Option<String>,
        }
        r.add(
            Spec::new("git.https.submit-credentials", "Submit https credentials", "git")
                .params(vec![ParamInfo::opt("username", ParamType::String), ParamInfo::opt("password", ParamType::String)])
                .doc("")
                .run(|e, v| async move {
                    let p: P = parse(v)?;
                    json_of(
                        wsl_cmds::submit_https_credentials(p.username, p.password, &e.git())
                            .await
                            .map_err(|()| crate::EngineError::Failed("submit_https_credentials falhou".into()))?,
                    )
                }),
        );
    }
    {
        #[allow(unused_imports)]
        use crate::git_cmds::wsl_cmds::*;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct P {}
        r.add(Spec::new("git.https.pending-credentials", "Pending https credentials", "git").params(vec![]).doc("").read_only().run(
            |e, v| async move {
                let p: P = parse(v)?;
                json_of(
                    wsl_cmds::pending_https_credentials(&e.git())
                        .await
                        .map_err(|()| crate::EngineError::Failed("pending_https_credentials falhou".into()))?,
                )
            },
        ));
    }
}

/// Quantos comandos este arquivo registra (conferido nos testes).
#[allow(dead_code)]
pub const COUNT: usize = 111;
