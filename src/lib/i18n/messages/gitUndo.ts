import type { Messages } from '../index.svelte'

export default {
  en: {
    undone: "Undone: {label}",
    redone: "Redone: {label}",
    undoTitle: "Undo the last git action: {action} ({key})",
    nothingToUndo: "No git action to undo",
    undo: "Undo",
    redoTitle: "Redo the undone git action: {action} ({key})",
    nothingToRedo: "No git action to redo",
    redo: "Redo",
  },
  pt: {
    undone: "Desfeito: {label}",
    redone: "Refeito: {label}",
    undoTitle: "Desfazer a última ação git: {action} ({key})",
    nothingToUndo: "Nenhuma ação git para desfazer",
    undo: "Desfazer",
    redoTitle: "Refazer a ação git desfeita: {action} ({key})",
    nothingToRedo: "Nenhuma ação git para refazer",
    redo: "Refazer",
  },
  es: {
    undone: "Deshecho: {label}",
    redone: "Rehecho: {label}",
    undoTitle: "Deshacer la última acción git: {action} ({key})",
    nothingToUndo: "No hay acción git para deshacer",
    undo: "Deshacer",
    redoTitle: "Rehacer la acción git deshecha: {action} ({key})",
    nothingToRedo: "No hay acción git para rehacer",
    redo: "Rehacer",
  },
  zh: {
    undone: "已撤销：{label}",
    redone: "已重做：{label}",
    undoTitle: "撤销上一个 git 操作：{action}（{key}）",
    nothingToUndo: "没有可撤销的 git 操作",
    undo: "撤销",
    redoTitle: "重做已撤销的 git 操作：{action}（{key}）",
    nothingToRedo: "没有可重做的 git 操作",
    redo: "重做",
  },
} satisfies Messages
