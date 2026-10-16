use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details")]
pub enum GitCommandError {
    #[error("repositório não encontrado ou inválido: {path}")]
    RepoNotFound { path: String },

    #[error("erro do git ({code}/{class}): {message}")]
    Libgit2 { code: String, class: String, message: String },

    #[error("erro interno: {0}")]
    Internal(String),

    #[error("push rejeitado: o remote tem commits que você não tem localmente ({message})")]
    PushRejectedNonFastForward { message: String },

    /// A background operation needed this SSH key's passphrase, which it may
    /// not ask for; an operation the user starts will.
    #[error("a chave SSH {key} está bloqueada: faça fetch ou push para digitar a senha dela")]
    SshPassphraseRequired { key: String },

    /// A background operation found no stored HTTPS credentials for this
    /// remote; like `SshPassphraseRequired`, only a user-started one asks.
    #[error("{url} precisa de usuário e senha/token: faça fetch ou push para informá-los")]
    HttpsCredentialsRequired { url: String },

    /// Which remote to push to is ambiguous (no upstream, no `origin`, several
    /// remotes) or there's none; the UI lets the user pick one.
    #[error("escolha um remote para o push")]
    RemoteRequired { remotes: Vec<String> },

    #[error("a branch {branch} não tem upstream: faça push com tracking ou defina branch.{branch}.merge")]
    NoUpstream { branch: String },

    /// Pull in fast-forward-only mode (`pull.ff=only` or the user's choice)
    /// met a diverged upstream; the UI offers merge/rebase instead.
    #[error("não é possível fazer fast-forward para {upstream}: as branches divergiram")]
    PullNotFastForward { upstream: String },

    /// A git hook (pre-commit, commit-msg...) exited non-zero and aborted the
    /// commit; `output` is what it printed, for the UI to show.
    #[error("o hook {hook} falhou")]
    HookFailed { hook: String, output: String },

    /// Signing was required (commit.gpgsign or the app setting) but failed.
    #[error("não foi possível assinar o commit: {message}")]
    SigningFailed { message: String },
}

impl From<git2::Error> for GitCommandError {
    fn from(e: git2::Error) -> Self {
        GitCommandError::Libgit2 { code: format!("{:?}", e.code()), class: format!("{:?}", e.class()), message: e.message().to_string() }
    }
}
