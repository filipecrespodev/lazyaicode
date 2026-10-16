import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import { getConfig } from "@/lib/config-store";
import { errorMessage } from "@/lib/tauri-client";
import { listInstalledEditors, openInEditorCmd, repoFilePath } from "./api";
import type { EditorInfo } from "./api";
import { t } from "../../../lib/i18n/react";

/** The editor to use: the saved choice if it's still installed, else null (system default). */
export function pickEditor(installed: EditorInfo[], savedId: string | null): EditorInfo | null {
  return installed.find((e) => e.id === savedId) ?? null;
}

export async function openFileInEditor(repoId: string, relPath: string) {
  try {
    const path = await repoFilePath(repoId, relPath);
    const editor = pickEditor(await listInstalledEditors(), await getConfig<string | null>("defaultEditor", null));
    if (editor) await openInEditorCmd(editor.command, path);
    else await openPath(path);
  } catch (e) {
    toast.error(errorMessage(e));
  }
}

export async function revealFileInManager(repoId: string, relPath: string) {
  try {
    await revealItemInDir(await repoFilePath(repoId, relPath));
  } catch (e) {
    toast.error(errorMessage(e));
  }
}

/** Context-menu entries shared by every file list. */
export const fileOpenMenuItems = (repoId: string, relPath: string) => [
  { label: t("gitEditor.openInEditor"), onClick: () => openFileInEditor(repoId, relPath) },
  { label: t("gitEditor.revealInFileManager"), onClick: () => revealFileInManager(repoId, relPath) },
];
