//! Workspaces: uma pasta que agrupa repositórios, com a
//! sua própria conta Claude. Aqui ficam as regras puras — quais repositórios
//! pertencem ao workspace e quais sessões ele importa.

use lazyaicode_doc::Workspace;
use std::path::{Path, PathBuf};

/// Repositório de um workspace (`name`, `path`).
pub use lazyaicode_git::host::WorkspaceRepoEntry as RepoEntry;

/// Repositórios git nos filhos imediatos da pasta; a própria pasta vem
/// primeiro quando também é um repositório (o usuário escolheu o repo, não
/// a pasta que o contém). Pasta de uma distro WSL: ver
/// `git_client::state::AppState::scan_workspace`, que varre dentro da distro.
pub fn scan_repos(path: &Path) -> Vec<RepoEntry> {
    lazyaicode_git::host::scan(path).unwrap_or_default()
}

/// Como `scan_repos`, mas uma pasta de distro WSL (`\\wsl.localhost\...`)
/// é varrida dentro da distro pelo `lazyaicode-agent`, sem atravessar o 9P.
/// Se o agente não subir, cai na varredura comum (lenta, mas funciona).
pub async fn scan_repos_routed(git: &lazyaicode_git::state::AppState, path: &str) -> Vec<RepoEntry> {
    match git.scan_workspace(path).await {
        Ok(repos) => repos,
        Err(err) => {
            lazyaicode_git::activity::error(None, format!("varredura de {path} pelo agente WSL falhou: {err}"));
            let path = PathBuf::from(path);
            tokio::task::spawn_blocking(move || scan_repos(&path)).await.unwrap_or_default()
        }
    }
}

/// A pasta está dentro do workspace (comparação por componentes, não por
/// prefixo de texto: `/a/web2` não está em `/a/web`).
pub fn contains(ws: &Workspace, path: &Path) -> bool {
    path.starts_with(&ws.path)
}

pub fn is_ignored(ws: &Workspace, path: &Path) -> bool {
    ws.ignored_paths.iter().any(|i| path.starts_with(i))
}

/// Uma sessão do Claude Code pertence ao workspace quando rodou numa pasta
/// dele que não está na lista de ignorados.
pub fn owns_session_cwd(ws: &Workspace, cwd: &Path) -> bool {
    contains(ws, cwd) && !is_ignored(ws, cwd)
}

pub fn claude_dir(ws: &Workspace) -> PathBuf {
    ws.claude_dir.as_deref().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(lazyaicode_agent::claude::default_claude_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(path: &str, ignored: &[&str]) -> Workspace {
        Workspace {
            id: "w".into(),
            name: "w".into(),
            path: path.into(),
            claude_dir: None,
            ignored_paths: ignored.iter().map(|s| s.to_string()).collect(),
            created_at: 0,
        }
    }

    #[test]
    fn lists_every_child_folder_marking_repos_and_the_folder_itself_when_it_is_a_repo() {
        let d = tempfile::tempdir().unwrap();
        for r in ["web", "Api", "docs", ".cache"] {
            std::fs::create_dir_all(d.path().join(r)).unwrap();
        }
        std::fs::write(d.path().join("README.md"), "").unwrap();
        std::fs::create_dir_all(d.path().join("web/.git")).unwrap();
        std::fs::create_dir_all(d.path().join("Api/.git")).unwrap();
        let found: Vec<_> = scan_repos(d.path()).into_iter().map(|r| (r.name, r.is_repo)).collect();
        assert_eq!(found, vec![("Api".into(), true), ("docs".into(), false), ("web".into(), true)]);

        std::fs::create_dir_all(d.path().join(".git")).unwrap();
        let all = scan_repos(d.path());
        assert_eq!(all.len(), 4);
        assert!(all[0].is_repo);
    }

    #[test]
    fn a_session_belongs_to_the_workspace_only_inside_its_folder_and_outside_ignored_repos() {
        let w = ws("/p/trevo", &["/p/trevo/legado"]);
        assert!(owns_session_cwd(&w, Path::new("/p/trevo/web")));
        assert!(owns_session_cwd(&w, Path::new("/p/trevo/web/src")));
        assert!(!owns_session_cwd(&w, Path::new("/p/trevo2/web")));
        assert!(!owns_session_cwd(&w, Path::new("/p/trevo/legado/x")));
        assert!(!owns_session_cwd(&w, Path::new("/outro")));
    }
}

/// Ajustes do processo antes de abrir qualquer coisa (PATH do WSL, backend
/// gráfico). Chamado pelos apps no início.
pub fn init_platform() {
    lazyaicode_platform::prepare_env();
}
