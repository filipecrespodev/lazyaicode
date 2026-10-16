//! Nomes dos eventos que o motor emite. O payload é JSON livre; a forma de
//! cada um está documentada em `book/src/automation/control-protocol.md`.

/// `{threadId, msg}`: uma linha do agente (stream-json do Claude Code ou o
/// ACP já traduzido para o mesmo formato).
pub const AGENT_EVENT: &str = "agent-event";
/// `{threadId, code, stderr}`: o processo do agente terminou.
pub const AGENT_EXIT: &str = "agent-exit";
/// `{threadId, status}`: idle | running | awaiting | error.
pub const THREAD_STATUS: &str = "thread-status";
/// A thread inteira, depois de uma mudança feita pelo motor.
pub const THREAD_UPDATED: &str = "thread-updated";
/// `{termId, data}`: bytes do PTY em base64; `data: null` = terminal fechou.
pub const TERM_DATA: &str = "term-data";
/// Workspaces, projetos ou threads mudaram (recarregar listas).
pub const STATE_CHANGED: &str = "state-changed";
/// `{level, message, repoId?}`: registro de atividade do cliente git.
pub const GIT_ACTIVITY: &str = "git-activity";
/// Pedidos de senha do ssh / credenciais https do cliente git.
pub const GIT_PASSPHRASE: &str = "ssh-passphrase-request";
pub const GIT_CREDENTIALS: &str = "https-credentials-request";
/// `{repoId}`: o repositório mudou no disco ou por uma operação.
pub const REPO_CHANGED: &str = "repo-changed";
/// `{text, kind}`: aviso para mostrar ao usuário (toast).
pub const NOTICE: &str = "notice";

/// Todos os eventos conhecidos, para o `events.list` e os testes.
pub const ALL: &[&str] = &[
    AGENT_EVENT,
    AGENT_EXIT,
    THREAD_STATUS,
    THREAD_UPDATED,
    TERM_DATA,
    STATE_CHANGED,
    GIT_ACTIVITY,
    GIT_PASSPHRASE,
    GIT_CREDENTIALS,
    REPO_CHANGED,
    NOTICE,
];
