import type { Messages } from '../index.svelte'

export default {
  en: {
    welcomeTitle: 'Welcome to LazyAICode',
    welcomeText:
      'Create a workspace: a folder with your repositories and the Claude account it uses. Each workspace becomes a tab at the top, and the Claude Code sessions you already have in that folder show up as threads.',
    createWorkspace: 'Create workspace',
    reposOne: '{n} repository',
    reposOther: '{n} repositories',
    account: 'account',
    notLoggedIn: 'This account is not logged in. See Settings → Claude accounts.',
    newThreadIn: 'New thread in {name}',
    palette: 'Palette',
    recent: 'Recent',
    updating: 'Updating LazyAICode to version {version}…',
  },
  pt: {
    welcomeTitle: 'Bem-vindo ao LazyAICode',
    welcomeText:
      'Crie um workspace: uma pasta com os seus repositórios e a conta Claude que ela usa. Cada workspace vira uma aba no topo, e as sessões do Claude Code que você já tem nessa pasta aparecem como threads.',
    createWorkspace: 'Criar workspace',
    reposOne: '{n} repositório',
    reposOther: '{n} repositórios',
    account: 'conta',
    notLoggedIn: 'Esta conta não está logada. Veja Configurações → Contas Claude.',
    newThreadIn: 'Nova thread em {name}',
    palette: 'Paleta',
    recent: 'Recentes',
    updating: 'Atualizando o LazyAICode para a versão {version}…',
  },
  es: {
    welcomeTitle: 'Bienvenido a LazyAICode',
    welcomeText:
      'Crea un workspace: una carpeta con tus repositorios y la cuenta de Claude que usa. Cada workspace se convierte en una pestaña arriba, y las sesiones de Claude Code que ya tienes en esa carpeta aparecen como threads.',
    createWorkspace: 'Crear workspace',
    reposOne: '{n} repositorio',
    reposOther: '{n} repositorios',
    account: 'cuenta',
    notLoggedIn: 'Esta cuenta no ha iniciado sesión. Ve a Configuración → Cuentas de Claude.',
    newThreadIn: 'Nueva thread en {name}',
    palette: 'Paleta',
    recent: 'Recientes',
    updating: 'Actualizando LazyAICode a la versión {version}…',
  },
  zh: {
    welcomeTitle: '欢迎使用 LazyAICode',
    welcomeText:
      '创建一个 workspace：包含你的仓库以及它所使用的 Claude 账号的文件夹。每个 workspace 都会成为顶部的一个标签页，该文件夹中已有的 Claude Code 会话会显示为 thread。',
    createWorkspace: '创建 workspace',
    reposOne: '{n} 个仓库',
    reposOther: '{n} 个仓库',
    account: '账号',
    notLoggedIn: '此账号未登录。请前往 设置 → Claude 账号。',
    newThreadIn: '在 {name} 中新建 thread',
    palette: '命令面板',
    recent: '最近',
    updating: '正在将 LazyAICode 更新到版本 {version}…',
  },
} satisfies Messages
