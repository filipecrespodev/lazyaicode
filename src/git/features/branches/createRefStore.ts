import { create } from "zustand";

/** Janela "Criar branch"/"Criar tag" aberta a partir de um menu (branch na
 * sidebar ou no grafo); renderizada uma vez só, na raiz global dos painéis. */
type CreateRef = { kind: "branch" | "tag"; repoId: string; start: string } | null;

export const useCreateRef = create<{ open: CreateRef }>(() => ({ open: null }));

export const openCreateRef = (open: NonNullable<CreateRef>) => useCreateRef.setState({ open });
export const closeCreateRef = () => useCreateRef.setState({ open: null });
