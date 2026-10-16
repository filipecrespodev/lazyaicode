import type { Messages } from '../index.svelte'

export default {
  en: {
    defaultEditor: 'Default editor',
    defaultEditorHint: 'Used by "Open in editor" on files. Detected from your PATH and the usual install locations.',
    systemDefault: 'System default app',
    noneFound: 'No known editor was found; the system default is used.',
    openInEditor: 'Open in editor',
    revealInFileManager: 'Show in file manager',
  },
  pt: {
    defaultEditor: 'Editor padrão',
    defaultEditorHint: 'Usado por "Abrir no editor" nos arquivos. Detectado a partir do seu PATH e dos locais de instalação habituais.',
    systemDefault: 'Aplicativo padrão do sistema',
    noneFound: 'Nenhum editor conhecido foi encontrado; o padrão do sistema é usado.',
    openInEditor: 'Abrir no editor',
    revealInFileManager: 'Mostrar no gerenciador de arquivos',
  },
  es: {
    defaultEditor: 'Editor predeterminado',
    defaultEditorHint: 'Lo usa "Abrir en el editor" en los archivos. Se detecta a partir de tu PATH y de las ubicaciones de instalación habituales.',
    systemDefault: 'Aplicación predeterminada del sistema',
    noneFound: 'No se encontró ningún editor conocido; se usa el predeterminado del sistema.',
    openInEditor: 'Abrir en el editor',
    revealInFileManager: 'Mostrar en el administrador de archivos',
  },
  zh: {
    defaultEditor: '默认编辑器',
    defaultEditorHint: '用于文件的“在编辑器中打开”。根据 PATH 和常见安装位置检测。',
    systemDefault: '系统默认应用',
    noneFound: '未找到已知的编辑器；将使用系统默认应用。',
    openInEditor: '在编辑器中打开',
    revealInFileManager: '在文件管理器中显示',
  },
} satisfies Messages
