import type { Messages } from '../index.svelte'

export default {
  en: {
    title: 'Commit signing',
    description: 'Repositories with {gpgsign} are always signed as the git config says ({format} ssh or openpgp, {key}), with ssh-keygen or gpg.',
    signAll: 'Sign all my commits with SSH',
    signAllHint: 'Uses each repository\'s SSH key (the repository\'s, else the workspace\'s, else the default). Your git config isn\'t changed. A passphrase-protected key is unlocked by ssh-agent or asked for once per session.',
  },
  pt: {
    title: 'Assinatura de commits',
    description: 'Repositórios com {gpgsign} são sempre assinados como manda a config do git ({format} ssh ou openpgp, {key}), com ssh-keygen ou gpg.',
    signAll: 'Assinar todos os meus commits com SSH',
    signAllHint: 'Usa a chave SSH de cada repositório (a do repositório, senão a do workspace, senão a padrão). Sua config do git não é alterada. Uma chave com senha é desbloqueada pelo ssh-agent ou pedida uma vez por sessão.',
  },
  es: {
    title: 'Firma de commits',
    description: 'Los repositorios con {gpgsign} siempre se firman según la config de git ({format} ssh u openpgp, {key}), con ssh-keygen o gpg.',
    signAll: 'Firmar todos mis commits con SSH',
    signAllHint: 'Usa la clave SSH de cada repositorio (la del repositorio, si no la del workspace, si no la predeterminada). Tu config de git no se modifica. Una clave con contraseña se desbloquea con ssh-agent o se pide una vez por sesión.',
  },
  zh: {
    title: '提交签名',
    description: '设置了 {gpgsign} 的仓库始终按 git 配置签名（{format} 为 ssh 或 openpgp，{key}），使用 ssh-keygen 或 gpg。',
    signAll: '使用 SSH 签名我的所有提交',
    signAllHint: '使用每个仓库的 SSH 密钥（优先仓库的，其次工作区的，最后默认的）。不会修改你的 git 配置。带密码的密钥由 ssh-agent 解锁，或每个会话询问一次。',
  },
} satisfies Messages
