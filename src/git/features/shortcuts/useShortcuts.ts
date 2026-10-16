import { useEffect, useRef, useSyncExternalStore } from "react";
import { bindingsFor, matches } from "../../../lib/shortcuts/shortcuts";
import { getOverrides, loadOverrides, onOverridesChange, resetBinding, setBinding } from "../../../lib/shortcuts/store.svelte";

/** The user's bindings (shared with the Svelte shell; changes apply live). */
export function useShortcutOverrides() {
  useEffect(() => void loadOverrides(), []);
  const overrides = useSyncExternalStore(onOverridesChange, getOverrides);
  return { overrides, setBinding, reset: resetBinding };
}

/** Runs `handler` when the action's current binding is pressed. The handler
 * decides about preventDefault and about where the key applies (e.g. not in
 * the terminal or a text field). */
export function useShortcut(id: string, handler: (e: KeyboardEvent) => void, enabled = true) {
  const { overrides } = useShortcutOverrides();
  const bindings = bindingsFor(id, overrides);
  const handlerRef = useRef(handler);
  handlerRef.current = handler;
  const bindingKey = bindings.join("|");

  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (bindings.some((b) => matches(e, b))) handlerRef.current(e);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, bindingKey]);
}
