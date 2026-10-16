import type { Messages } from '../index.svelte'

export default {
  en: {
    pullDone: 'git lfs pull finished',
    bannerInstalled: 'This repository uses Git LFS. Checkouts made by LazyAICode leave LFS pointer files in place of the real content until you pull them.',
    bannerNotInstalled: 'This repository uses Git LFS. Checkouts made by LazyAICode leave LFS pointer files in place of the real content — and git-lfs isn\'t installed; install it to download the content.',
    runPull: 'Run git lfs pull',
    hide: 'Hide for this session',
  },
  pt: {
    pullDone: 'git lfs pull concluído',
    bannerInstalled: 'Este repositório usa Git LFS. Checkouts feitos pelo LazyAICode deixam arquivos ponteiro do LFS no lugar do conteúdo real até você fazer pull deles.',
    bannerNotInstalled: 'Este repositório usa Git LFS. Checkouts feitos pelo LazyAICode deixam arquivos ponteiro do LFS no lugar do conteúdo real — e o git-lfs não está instalado; instale-o para baixar o conteúdo.',
    runPull: 'Executar git lfs pull',
    hide: 'Ocultar nesta sessão',
  },
  es: {
    pullDone: 'git lfs pull completado',
    bannerInstalled: 'Este repositorio usa Git LFS. Los checkouts hechos por LazyAICode dejan archivos puntero de LFS en lugar del contenido real hasta que hagas pull de ellos.',
    bannerNotInstalled: 'Este repositorio usa Git LFS. Los checkouts hechos por LazyAICode dejan archivos puntero de LFS en lugar del contenido real — y git-lfs no está instalado; instálalo para descargar el contenido.',
    runPull: 'Ejecutar git lfs pull',
    hide: 'Ocultar en esta sesión',
  },
  zh: {
    pullDone: 'git lfs pull 已完成',
    bannerInstalled: '此仓库使用 Git LFS。由 LazyAICode 执行的检出会留下 LFS 指针文件而非真实内容，直到你拉取它们。',
    bannerNotInstalled: '此仓库使用 Git LFS。由 LazyAICode 执行的检出会留下 LFS 指针文件而非真实内容 — 且未安装 git-lfs；请安装后下载内容。',
    runPull: '运行 git lfs pull',
    hide: '本次会话中隐藏',
  },
} satisfies Messages
