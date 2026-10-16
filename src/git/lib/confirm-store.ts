import { create } from "zustand";

type ConfirmKind = "info" | "warning" | "error";

type ConfirmRequest = {
  message: string;
  title?: string;
  kind: ConfirmKind;
  resolve: (value: boolean) => void;
};

type ConfirmStore = {
  request: ConfirmRequest | null;
  setRequest: (request: ConfirmRequest | null) => void;
};

export const useConfirmStore = create<ConfirmStore>((set) => ({
  request: null,
  setRequest: (request) => set({ request }),
}));

/** In-app replacement for `@tauri-apps/plugin-dialog`'s `confirm()` — same
 * signature, but rendered as a themed component instead of a native OS
 * window. Rendered once by `ConfirmDialogHost`, mounted at the app root. */
export function confirm(
  message: string,
  opts?: { title?: string; kind?: ConfirmKind },
): Promise<boolean> {
  return new Promise((resolve) => {
    useConfirmStore.getState().setRequest({
      message,
      title: opts?.title,
      kind: opts?.kind ?? "info",
      resolve,
    });
  });
}
