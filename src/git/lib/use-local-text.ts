import { useEffect, useRef, useState } from "react";

/** Local, same-tick echo of a value that's otherwise backed by an external
 * store (React Query cache, disk...). The Tauri Linux webview (WebKitGTK)
 * resets a controlled input's cursor to the end on any re-render that isn't
 * in the exact same synchronous tick as the keystroke — even a microtask
 * later, which is what TanStack Query's notifyManager batching does. Local
 * state updates synchronously within the keystroke's own event handler, so
 * the cursor never moves; `onCommit` persists in the background. */
export function useLocalText(value: string, onCommit: (next: string) => void) {
  const [local, setLocal] = useState(value);
  const lastCommitted = useRef(value);

  useEffect(() => {
    if (value !== lastCommitted.current) {
      lastCommitted.current = value;
      setLocal(value);
    }
  }, [value]);

  const onChange = (next: string) => {
    lastCommitted.current = next;
    setLocal(next);
    onCommit(next);
  };

  return [local, onChange] as const;
}
