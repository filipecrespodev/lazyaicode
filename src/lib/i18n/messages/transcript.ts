import type { Messages } from '../index.svelte'

export default {
  en: { image: '[image]', taskOne: '{n} task', taskOther: '{n} tasks' },
  pt: { image: '[imagem]', taskOne: '{n} tarefa', taskOther: '{n} tarefas' },
  es: { image: '[imagen]', taskOne: '{n} tarea', taskOther: '{n} tareas' },
  zh: { image: '[图片]', taskOne: '{n} 个任务', taskOther: '{n} 个任务' },
} satisfies Messages
