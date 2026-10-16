import { call } from "@/lib/tauri-client";

export type EditorInfo = { id: string; name: string; command: string };

export const listInstalledEditors = () => call<EditorInfo[]>("list_installed_editors");
export const repoFilePath = (repoId: string, relPath: string) =>
  call<string>("repo_file_path", { repoId, relPath });
export const openInEditorCmd = (command: string, path: string) =>
  call<void>("open_in_editor", { command, path });
