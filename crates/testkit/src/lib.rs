//! Ajudantes de teste compartilhados pelos crates e apps.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[cfg(unix)]
pub mod fakes;
pub mod scenario;

use std::path::Path;
use std::process::Command;

/// Roda `git` em `dir` e falha o teste se der erro.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().expect("git no PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Um repositório com um commit (`a.txt` = "um\n") na branch `main`.
pub fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "T"]);
    git(dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("a.txt"), "um\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "first"]);
}

/// Um `claude` falso (script sh) que anuncia a sessão `sess-1` e, a cada
/// mensagem do usuário, responde `reply` e fecha o turno. Só Unix.
#[cfg(unix)]
pub fn fake_claude(dir: &Path, reply: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("fake-claude");
    let reply = reply.replace('\\', "\\\\").replace('"', "\\\"").replace('\'', "");
    std::fs::write(
        &bin,
        format!(
            "#!/bin/sh\nprintf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"sess-1\"}}'\n\
             while read -r l; do\n  case \"$l\" in *'\"type\":\"user\"'*)\n\
             printf '%s\\n' '{{\"type\":\"assistant\",\"message\":{{\"id\":\"m1\",\"content\":[{{\"type\":\"text\",\"text\":\"{reply}\"}}]}}}}'\n\
             printf '%s\\n' '{{\"type\":\"result\",\"is_error\":false,\"duration_ms\":5,\"total_cost_usd\":0}}';;\n  esac\ndone\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}
