//! Git: checkouts isolados por thread, diffs para revisão e o caminho de volta
//! das mudanças para o projeto. Tudo via o binário `git`, sem libgit2, para
//! respeitar hooks, config e credenciais do usuário.

use anyhow::{anyhow, bail, Context, Result};
use lazyaicode_platform::command;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

pub fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    git_env(cwd, args, &[])
}

fn git_env(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String> {
    let mut cmd = command("git");
    cmd.current_dir(cwd).args(args).env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().with_context(|| format!("git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!("git {} falhou: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn repo_root(path: &Path) -> Result<PathBuf> {
    let out = git(path, &["rev-parse", "--show-toplevel"]).map_err(|_| anyhow!("{} não é um repositório git", path.display()))?;
    Ok(PathBuf::from(out.trim()))
}

/// `git fetch` com limite de tempo: um remoto que pede senha ou não responde
/// não pode travar a criação da thread. Falha é aceitável — parte-se dos
/// commits locais.
fn fetch_with_timeout(repo: &Path, secs: u64) {
    let child = command("git")
        .current_dir(repo)
        .args(["fetch", "--quiet", "origin"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return };
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
}

/// Branch padrão do remoto (`origin/HEAD`), ou `HEAD` local sem remoto.
pub fn default_base(repo: &Path) -> String {
    if let Ok(r) = git(repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
        return r.trim().to_string();
    }
    for c in ["origin/main", "origin/master"] {
        if git(repo, &["rev-parse", "--verify", "--quiet", c]).is_ok() {
            return c.to_string();
        }
    }
    "HEAD".into()
}

pub struct Isolated {
    pub path: PathBuf,
    pub branch: String,
    pub base_ref: String,
}

/// Cria um checkout isolado (git worktree) fora do repositório, numa branch
/// nova a partir da branch padrão do remoto. O checkout principal do usuário
/// não muda de branch nem de arquivos.
pub fn create_isolated(repo: &Path, dest: &Path, short_id: &str, fetch: bool) -> Result<Isolated> {
    if fetch {
        fetch_with_timeout(repo, 15);
    }
    let base_ref = default_base(repo);
    let branch = format!("lazy/{short_id}");
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let dest_s = dest.to_string_lossy().to_string();
    git(repo, &["worktree", "add", "-b", &branch, &dest_s, &base_ref])?;
    link_shared_files(repo, dest);
    run_prepare(dest);
    Ok(Isolated { path: dest.to_path_buf(), branch, base_ref })
}

/// `.agents/linked` (ou `.delta/linked`): um caminho por linha, ignorado pelo
/// git (ex.: `.env`), que vira symlink para a cópia do checkout principal.
fn link_shared_files(repo: &Path, dest: &Path) {
    for list in [".agents/linked", ".delta/linked", ".lazyaicode/linked"] {
        let Ok(content) = std::fs::read_to_string(repo.join(list)) else { continue };
        for line in content.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || line.contains("..") {
                continue;
            }
            let src = repo.join(line);
            let dst = dest.join(line);
            if src.exists() && !dst.exists() {
                if let Some(p) = dst.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&src, &dst);
            }
        }
    }
}

/// `.agents/prepare` (ou `.delta/prepare`): script rápido de preparação do
/// checkout, rodado uma vez na criação.
fn run_prepare(dest: &Path) {
    for script in [".agents/prepare", ".delta/prepare", ".lazyaicode/prepare"] {
        let p = dest.join(script);
        if p.is_file() {
            let _ = command("sh").arg(&p).current_dir(dest).output();
            return;
        }
    }
}

pub fn remove_isolated(repo: &Path, path: &Path) -> Result<()> {
    if path.exists() {
        git(repo, &["worktree", "remove", "--force", &path.to_string_lossy()])?;
    }
    let _ = git(repo, &["worktree", "prune"]);
    Ok(())
}

/// Árvore git com o estado atual da pasta — incluindo arquivos não rastreados
/// que o `.gitignore` não exclui — sem tocar no índice real: usa uma cópia do
/// índice em `GIT_INDEX_FILE`.
pub fn snapshot_tree(cwd: &Path) -> Result<String> {
    let index = git(cwd, &["rev-parse", "--path-format=absolute", "--git-path", "index"])?;
    let index = PathBuf::from(index.trim());
    let tmp = std::env::temp_dir().join(format!("lazyaicode-index-{}-{}", std::process::id(), uuid::Uuid::new_v4().simple()));
    if index.exists() {
        std::fs::copy(&index, &tmp)?;
    }
    let tmp_s = tmp.to_string_lossy().to_string();
    let env = [("GIT_INDEX_FILE", tmp_s.as_str())];
    let res = git_env(cwd, &["add", "-A"], &env).and_then(|_| git_env(cwd, &["write-tree"], &env));
    let _ = std::fs::remove_file(&tmp);
    Ok(res?.trim().to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: String,
    pub binary: bool,
    pub truncated: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Changes {
    pub base: String,
    pub base_label: String,
    pub files: Vec<FileChange>,
}

const MAX_PATCH: usize = 300_000;

fn fingerprint(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Separa a saída de `git diff` em um bloco por arquivo.
pub fn split_patch(diff: &str) -> Vec<FileChange> {
    let mut files = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let flush = |cur: &mut Vec<&str>, files: &mut Vec<FileChange>| {
        if cur.is_empty() {
            return;
        }
        let text = cur.join("\n");
        files.push(parse_file_patch(&text));
        cur.clear();
    };
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            flush(&mut cur, &mut files);
        }
        cur.push(line);
    }
    flush(&mut cur, &mut files);
    files
}

fn strip_prefix_path(p: &str) -> String {
    let p = p.trim().trim_matches('"');
    p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p).to_string()
}

fn parse_file_patch(text: &str) -> FileChange {
    let mut path = String::new();
    let mut old_path = None;
    let mut status = "M".to_string();
    let mut additions = 0;
    let mut deletions = 0;
    let mut binary = false;
    let mut in_hunk = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // `a/x b/x` — com nomes sem espaço basta dividir ao meio
            if let Some(idx) = rest.find(" b/") {
                path = strip_prefix_path(&rest[idx + 1..]);
            }
        } else if in_hunk {
            if line.starts_with('+') {
                additions += 1;
            } else if line.starts_with('-') {
                deletions += 1;
            }
        } else if line.starts_with("new file") {
            status = "A".into();
        } else if line.starts_with("deleted file") {
            status = "D".into();
        } else if let Some(p) = line.strip_prefix("rename from ") {
            old_path = Some(p.to_string());
            status = "R".into();
        } else if let Some(p) = line.strip_prefix("rename to ") {
            path = p.to_string();
        } else if let Some(p) = line.strip_prefix("+++ ") {
            if p != "/dev/null" {
                path = strip_prefix_path(p);
            }
        } else if line.starts_with("Binary files") {
            binary = true;
        } else if line.starts_with("@@") {
            in_hunk = true;
        }
    }
    let truncated = text.len() > MAX_PATCH;
    let patch = if truncated {
        let mut end = MAX_PATCH;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].to_string()
    } else {
        text.to_string()
    };
    FileChange { fingerprint: fingerprint(text), path, old_path, status, additions, deletions, patch, binary, truncated }
}

/// Resolve a base pedida pela UI em um commit/árvore comparável.
/// `upstream` = merge-base com a ref de origem; `head` = último commit;
/// `turn` = árvore salva no início do último turno.
pub fn resolve_base(cwd: &Path, kind: &str, base_ref: Option<&str>, turn_tree: Option<&str>) -> Result<(String, String)> {
    match kind {
        "turn" => {
            let t = turn_tree.ok_or_else(|| anyhow!("Nenhum turno registrado ainda"))?;
            Ok((t.to_string(), "Último turno".into()))
        }
        "head" => Ok((git(cwd, &["rev-parse", "HEAD"])?.trim().to_string(), "Último commit".into())),
        _ => {
            let r = base_ref.map(str::to_string).unwrap_or_else(|| default_base(cwd));
            let mb = git(cwd, &["merge-base", "HEAD", &r]).or_else(|_| git(cwd, &["rev-parse", "HEAD"]))?;
            Ok((mb.trim().to_string(), r))
        }
    }
}

pub fn changes(cwd: &Path, base: &str, label: &str) -> Result<Changes> {
    let tree = snapshot_tree(cwd)?;
    let diff = git(cwd, &["diff", "--no-color", "--no-ext-diff", "-M", base, &tree])?;
    Ok(Changes { base: base.to_string(), base_label: label.to_string(), files: split_patch(&diff) })
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    pub base_ref: String,
}

pub fn branch_info(cwd: &Path, base_ref: Option<&str>) -> Result<BranchInfo> {
    let branch = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string();
    let base = base_ref.map(str::to_string).unwrap_or_else(|| default_base(cwd));
    let (mut ahead, mut behind) = (0, 0);
    if let Ok(out) = git(cwd, &["rev-list", "--left-right", "--count", &format!("{base}...HEAD")]) {
        let mut it = out.split_whitespace();
        behind = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        ahead = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    }
    let dirty = !git(cwd, &["status", "--porcelain"])?.trim().is_empty();
    Ok(BranchInfo { branch, ahead, behind, dirty, base_ref: base })
}

/// `git add -A` e commit (o `thread.commit` e o `git.path.commit-all`; na
/// interface o commit é pelo staging da aba Histórico).
pub fn commit_all(cwd: &Path, message: &str) -> Result<String> {
    git(cwd, &["add", "-A"])?;
    if git(cwd, &["diff", "--cached", "--quiet"]).is_ok() {
        bail!("Nada para commitar");
    }
    git(cwd, &["commit", "-m", message])?;
    Ok(git(cwd, &["rev-parse", "--short", "HEAD"])?.trim().to_string())
}

/// Faz merge da branch da thread na branch atual do checkout principal.
/// Recusa se o checkout principal tiver mudanças rastreadas não commitadas.
pub fn merge_into(repo: &Path, branch: &str) -> Result<String> {
    if !git(repo, &["status", "--porcelain", "--untracked-files=no"])?.trim().is_empty() {
        bail!("O projeto tem mudanças não commitadas. Faça commit ou stash antes do merge.");
    }
    let target = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string();
    match git(repo, &["merge", "--no-ff", "--no-edit", branch]) {
        Ok(out) => Ok(format!("Merge de {branch} em {target} concluído.\n{}", out.trim())),
        Err(e) => {
            let _ = git(repo, &["merge", "--abort"]);
            Err(e)
        }
    }
}

/// Aplica as mudanças da thread (desde a base) como alterações não
/// commitadas no checkout principal, com merge de 3 vias quando necessário.
pub fn apply_into(cwd: &Path, repo: &Path, base: &str) -> Result<String> {
    let tree = snapshot_tree(cwd)?;
    let patch = git(cwd, &["diff", "--binary", "--no-color", "--no-ext-diff", base, &tree])?;
    if patch.trim().is_empty() {
        bail!("Nenhuma mudança para aplicar");
    }
    let tmp = std::env::temp_dir().join(format!("lazyaicode-{}.patch", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, &patch)?;
    let res = git(repo, &["apply", "--3way", "--whitespace=nowarn", &tmp.to_string_lossy()]);
    let _ = std::fs::remove_file(&tmp);
    res?;
    let n = split_patch(&patch).len();
    Ok(format!("{n} arquivo(s) aplicados ao projeto como mudanças não commitadas (novos arquivos ficam no stage)."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        git(p, &["init", "-q", "-b", "main"]).unwrap();
        git(p, &["config", "user.email", "t@t"]).unwrap();
        git(p, &["config", "user.name", "t"]).unwrap();
        std::fs::write(p.join("a.txt"), "um\ndois\n").unwrap();
        std::fs::write(p.join(".gitignore"), "segredo.env\n").unwrap();
        git(p, &["add", "-A"]).unwrap();
        git(p, &["commit", "-q", "-m", "init"]).unwrap();
        d
    }

    #[test]
    fn isolated_checkout_diff_includes_untracked_but_not_ignored_files() {
        let r = repo();
        let dest = r.path().parent().unwrap().join(format!("wt-{}", uuid::Uuid::new_v4().simple()));
        let iso = create_isolated(r.path(), &dest, "abc123", false).unwrap();
        assert_eq!(iso.branch, "lazy/abc123");
        assert_eq!(iso.base_ref, "HEAD");

        std::fs::write(dest.join("a.txt"), "um\ndois\ntres\n").unwrap();
        std::fs::write(dest.join("novo.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dest.join("segredo.env"), "X=1\n").unwrap();

        let (base, _) = resolve_base(&dest, "upstream", Some("main"), None).unwrap();
        let ch = changes(&dest, &base, "main").unwrap();
        let mut paths: Vec<_> = ch.files.iter().map(|f| (f.path.as_str(), f.status.as_str())).collect();
        paths.sort();
        assert_eq!(paths, vec![("a.txt", "M"), ("novo.rs", "A")]);
        let a = ch.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!((a.additions, a.deletions), (1, 0));

        // o índice real do checkout não foi tocado
        assert!(git(&dest, &["diff", "--cached", "--name-only"]).unwrap().trim().is_empty());
        // o checkout principal continua intacto
        assert_eq!(std::fs::read_to_string(r.path().join("a.txt")).unwrap(), "um\ndois\n");

        remove_isolated(r.path(), &dest).unwrap();
        assert!(!dest.exists());
    }

    #[test]
    fn turn_base_shows_only_what_changed_since_the_snapshot() {
        let r = repo();
        let p = r.path();
        std::fs::write(p.join("antes.txt"), "x\n").unwrap();
        let turn = snapshot_tree(p).unwrap();
        std::fs::write(p.join("depois.txt"), "y\n").unwrap();
        let ch = changes(p, &turn, "turn").unwrap();
        let paths: Vec<_> = ch.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["depois.txt"]);
    }

    #[test]
    fn fingerprint_changes_when_the_file_diff_changes() {
        let r = repo();
        let p = r.path();
        std::fs::write(p.join("a.txt"), "um\n").unwrap();
        let f1 = changes(p, "HEAD", "").unwrap().files[0].fingerprint.clone();
        std::fs::write(p.join("a.txt"), "um\nquatro\n").unwrap();
        let f2 = changes(p, "HEAD", "").unwrap().files[0].fingerprint.clone();
        assert_ne!(f1, f2);
    }

    #[test]
    fn merge_and_apply_bring_work_back_to_the_project() {
        let r = repo();
        let dest = r.path().parent().unwrap().join(format!("wt-{}", uuid::Uuid::new_v4().simple()));
        let iso = create_isolated(r.path(), &dest, "m1", false).unwrap();
        std::fs::write(dest.join("feito.txt"), "ok\n").unwrap();

        let (base, _) = resolve_base(&dest, "upstream", Some("main"), None).unwrap();
        apply_into(&dest, r.path(), &base).unwrap();
        assert!(r.path().join("feito.txt").exists());
        git(r.path(), &["reset", "-q", "--hard"]).unwrap();
        assert!(!r.path().join("feito.txt").exists());

        commit_all(&dest, "trabalho").unwrap();
        merge_into(r.path(), &iso.branch).unwrap();
        assert!(r.path().join("feito.txt").exists());
        remove_isolated(r.path(), &dest).unwrap();
    }

    #[test]
    fn merge_refuses_a_dirty_project() {
        let r = repo();
        std::fs::write(r.path().join("a.txt"), "sujo\n").unwrap();
        let err = merge_into(r.path(), "main").unwrap_err().to_string();
        assert!(err.contains("não commitadas"));
    }
}
