#!/usr/bin/env python3
"""Gera crates/engine/src/git_registry.rs a partir das funções públicas de
crates/engine/src/git_cmds/*.rs. Cada `pub async fn nome(args…, state: &AppState)`
vira um comando `git.<área>.<nome>` com os parâmetros em camelCase.

Rodar de novo depois de adicionar ou mudar uma função em git_cmds:
    python3 xtask/scripts/gen_git_commands.py
"""
import re, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, "crates/engine/src/git_cmds")
OUT = os.path.join(ROOT, "crates/engine/src/git_registry.rs")

AREA = {
    "ai_cmds": "ai", "branch_cmds": "branch", "config_cmds": "config", "diff_cmds": "diff",
    "github_cmds": "github", "gitlab_cmds": "gitlab", "history_cmds": "history",
    "interactive_rebase_cmds": "irebase", "log_cmds": "log", "pull_cmds": "pull",
    "rebase_cmds": "rebase", "remote_cmds": "remote", "repo_cmds": "repo",
    "repo_tools_cmds": "tools", "stash_cmds": "stash", "undo_cmds": "undo",
    "working_cmds": "working", "wsl_cmds": "wsl",
}
# Funções que só leem (o MCP marca como seguras).
READ_PREFIX = ("get_", "list_", "search_", "pending_", "repo_file_path", "wsl_home")
DESTRUCTIVE = {"discard_path", "discard_all", "reset_to_commit", "reset_to_ref", "delete_local_branch",
               "delete_remote_branch", "delete_branch_both", "delete_local_tag", "delete_remote_tag",
               "stash_drop", "remove_worktree", "remove_remote", "sync_branch_to_remote", "abort_rebase"}

def split_args(s):
    out, depth, cur = [], 0, ""
    for c in s:
        if c in "<([": depth += 1
        if c in ">)]": depth -= 1
        if c == "," and depth == 0:
            out.append(cur); cur = ""
        else:
            cur += c
    if cur.strip(): out.append(cur)
    return [a.strip() for a in out if a.strip()]

def camel(n):
    p = n.split("_")
    return p[0] + "".join(x.title() for x in p[1:])

def ptype(t):
    t = t.strip()
    if t.startswith("Option<"):
        return ptype(t[7:-1])[0], False
    return {
        "String": "ParamType::String", "bool": "ParamType::Bool", "u32": "ParamType::Int", "u64": "ParamType::Int",
        "usize": "ParamType::Int", "i64": "ParamType::Int", "i32": "ParamType::Int", "u16": "ParamType::Int",
        "f64": "ParamType::Number", "Vec<String>": "ParamType::Strings",
    }.get(t, "ParamType::Json"), True

def short(area, name):
    n = name
    for a in (area, area.rstrip("s")):
        n = re.sub(rf"(^|_){a}(_|$)", r"\1\2", n).strip("_")
        n = n.replace("__", "_")
    return (n or name).replace("_", "-")

SKIP = {"auth_mode"}  # helpers públicos que não são comandos

# ids legíveis: o padrão é `git.<área>.<nome sem a área>`
RENAME = dict(
    list_branches="git.branch.list", list_tags="git.tag.list", create_tag="git.tag.create",
    delete_local_tag="git.tag.delete-local", push_tag="git.tag.push", delete_remote_tag="git.tag.delete-remote",
    list_remote_tags="git.tag.list-remote", cherry_pick="git.commit.cherry-pick", revert_commit="git.commit.revert",
    reset_to_commit="git.reset.to-commit", reset_to_ref="git.reset.to-ref",
    checkout_commit_detached="git.branch.checkout-detached",
    get_global_git_identity="git.config.identity", list_installed_editors="git.config.editors",
    list_system_fonts="system.fonts", get_commit_diff="git.diff.commit", get_range_diff="git.diff.range",
    get_file_content="git.diff.file-content", get_file_history="git.history.file",
    get_commit_file_diff="git.history.commit-file-diff", get_blame="git.history.blame",
    get_branch_compare="git.history.compare", get_tree_diff="git.history.tree-diff",
    get_rebase_plan="git.irebase.plan", start_interactive_rebase="git.irebase.start",
    get_commit_log="git.log.list", get_head_commit="git.log.head", get_commit_signature="git.log.signature",
    get_repo_stats="git.log.stats", get_reflog="git.reflog.list", search_commits="git.log.search",
    get_repo_fingerprint="git.repo.fingerprint", get_upstream_status="git.pull.upstream-status",
    pull_branch="git.pull.pull", get_rebase_status="git.rebase.status", list_remotes="git.remote.list",
    get_remote_url="git.remote.url", push_branch="git.remote.push", fetch_remote="git.remote.fetch",
    open_repository="git.repo.open", get_repo_status="git.repo.status", close_repository="git.repo.close",
    list_worktrees="git.worktree.list", add_worktree="git.worktree.add", remove_worktree="git.worktree.remove",
    prune_worktrees="git.worktree.prune", list_submodules="git.submodule.list",
    get_submodule_count="git.submodule.count", update_submodules="git.submodule.update",
    get_lfs_status="git.lfs.status", run_lfs_pull="git.lfs.pull", undo_last_action="git.undo.undo",
    redo_last_action="git.undo.redo", get_undo_status="git.undo.status", get_working_status="git.working.status",
    get_workdir_diff="git.working.diff", git_commit_changes="git.working.commit",
    amend_commit_message="git.working.amend-message", add_ignore_pattern="git.working.ignore",
    list_ssh_keys="git.ssh.list-keys", submit_ssh_passphrase="git.ssh.submit-passphrase",
    pending_ssh_passphrase="git.ssh.pending-passphrase", submit_https_credentials="git.https.submit-credentials",
    pending_https_credentials="git.https.pending-credentials", list_wsl_distros="wsl.distros", wsl_home="wsl.home",
    list_wsl_dirs="wsl.dirs", github_list_open_pull_requests="github.pr.list",
    github_create_pull_request="github.pr.create", github_list_assignees="github.assignees",
    github_list_repo_events="github.events", gitlab_get_project="gitlab.project",
    gitlab_list_open_merge_requests="gitlab.mr.list", gitlab_create_merge_request="gitlab.mr.create",
    gitlab_search_members="gitlab.members", gitlab_list_project_events="gitlab.events",
    ai_generate_commit_message="ai.commit-message", ai_compose_commits="ai.compose-commits",
    apply_composed_commits="git.working.apply-composed", ai_resolve_conflict="ai.resolve-conflict",
    ai_generate_pr_description="ai.pr-description", stash_save="git.stash.save", stash_list="git.stash.list",
    stash_pop="git.stash.pop", stash_apply="git.stash.apply", stash_drop="git.stash.drop",
)

def label(name):
    return name.replace("_", " ").capitalize()

fns = []
for f in sorted(os.listdir(SRC)):
    if not f.endswith(".rs") or f == "mod.rs": continue
    mod = f[:-3]
    src = open(os.path.join(SRC, f)).read()
    for m in (x for x in re.finditer(r"((?:\s*///[^\n]*\n)*)\s*pub (async )?fn (\w+)\s*\(([^{]*?)\)\s*(?:->\s*([^{]+))?\{", src) if x.group(3) not in SKIP):
        doc, is_async, name, args, ret = m.group(1), bool(m.group(2)), m.group(3), m.group(4), (m.group(5) or "").strip()
        params, call = [], []
        for a in split_args(args):
            an, at = [x.strip() for x in a.split(":", 1)]
            if at == "&AppState":
                call.append("e.git()"); continue
            if at == "&crate::Engine":
                call.append("&e"); continue
            params.append((an, at))
            call.append(f"p.{an}")
        doc = " ".join(l.strip()[3:].strip() for l in doc.strip().splitlines() if l.strip().startswith("///"))
        fns.append((mod, name, is_async, params, call, ret, doc))

lines = [
    "//! GERADO por `xtask/scripts/gen_git_commands.py` a partir de `git_cmds/*.rs`.",
    "//! Não editar à mão: mude a função em `git_cmds` e rode o script de novo.",
    "",
    "#![allow(unused_variables, clippy::too_many_lines)]",
    "",
    "use lazyaicode_protocol::{ParamInfo, ParamType};",
    "use serde::Deserialize;",
    "",
    "use crate::git_cmds::*;",
    "use crate::registry::{json_of, parse, Registry, Spec};",
    "",
    "#[allow(clippy::too_many_lines)]",
    "pub fn register(r: &mut Registry) {",
]
for mod, name, is_async, params, call, ret, doc in fns:
    area = AREA[mod]
    cid = RENAME.get(name) or f"git.{area}.{short(area, name)}"
    pinfo = ", ".join(
        f'ParamInfo::{"req" if req else "opt"}("{camel(n)}", {ty})'
        for n, t in params for ty, req in [ptype(t)]
    )
    ro = name.startswith(READ_PREFIX)
    fields = "".join(f"        {n}: {t},\n" for n, t in params)
    await_ = ".await" if is_async else ""
    result_ok = ret.startswith("Result")
    body_call = f"{mod}::{name}({', '.join(call)}){await_}"
    if result_ok and ", ()>" in ret:
        body = f"json_of({body_call}.map_err(|()| crate::EngineError::Failed(\"{name} falhou\".into()))?)"
    elif result_ok:
        body = f"json_of({body_call}.map_err(crate::EngineError::from)?)"
    elif ret:
        body = f"json_of({body_call})"
    else:
        body = f"{{ {body_call}; Ok(serde_json::Value::Null) }}"
    lines.append(f"    {{")
    lines.append(f"        #[allow(unused_imports)]")
    lines.append(f"        use crate::git_cmds::{mod}::*;")
    lines.append(f"        #[derive(Deserialize)]")
    lines.append(f"        #[serde(rename_all = \"camelCase\")]")
    lines.append(f"        #[allow(dead_code)]")
    lines.append(f"        struct P {{")
    lines.append(fields.rstrip("\n") if fields else "")
    lines.append(f"        }}")
    spec = f'Spec::new("{cid}", "cmd.{cid}", "git.{area}").params(vec![{pinfo}]).doc({doc!r})'.replace("'", '"') if False else None
    d = doc.replace("\\", "\\\\").replace('"', '\\"')
    lines.append(f'        r.add(Spec::new("{cid}", "{label(name)}", "{cid.split('.')[0]}").params(vec![{pinfo}]).doc("{d}"){".read_only()" if ro else ""}{".destructive()" if name in DESTRUCTIVE else ""}.run(|e, v| async move {{')
    lines.append(f"            let p: P = parse(v)?;")
    lines.append(f"            {body}")
    lines.append(f"        }}));")
    lines.append(f"    }}")
lines.append("}")
lines.append("")
lines.append(f"/// Quantos comandos este arquivo registra (conferido nos testes).")
lines.append("#[allow(dead_code)]")
lines.append(f"pub const COUNT: usize = {len(fns)};")
open(OUT, "w").write("\n".join(l for l in lines if l is not None) + "\n")
print(f"{len(fns)} comandos git -> {os.path.relpath(OUT, ROOT)}")
ids = [RENAME.get(n) or f"git.{AREA[m]}.{short(AREA[m], n)}" for m, n, *_ in fns]
assert len(ids) == len(set(ids)), "ids repetidos"
print("\n".join("  " + i for i in sorted(ids)))
