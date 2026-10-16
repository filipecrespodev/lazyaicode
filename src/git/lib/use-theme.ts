import { useEffect, useState } from "react";

// O tema é do app inteiro (src/lib/theme.ts), que grava `data-mode`
// ("dark" | "light") no <html>. Os painéis git só precisam saber o modo.

export type ThemeMode = "dark" | "light";

const read = (): ThemeMode => (document.documentElement.dataset.mode === "light" ? "light" : "dark");

/** Modo (claro/escuro) do tema aplicado, atualizado quando o tema muda. */
export function useThemeMode(): ThemeMode {
  const [mode, setMode] = useState<ThemeMode>(read);
  useEffect(() => {
    const observer = new MutationObserver(() => setMode(read()));
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-mode"] });
    return () => observer.disconnect();
  }, []);
  return mode;
}
