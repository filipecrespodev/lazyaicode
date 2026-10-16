import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

export function useTauriEvent<T>(event: string, onEvent: (payload: T) => void) {
  const handlerRef = useRef(onEvent);
  handlerRef.current = onEvent;

  useEffect(() => {
    const unlisten = listen<T>(event, (e) => handlerRef.current(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, [event]);
}
