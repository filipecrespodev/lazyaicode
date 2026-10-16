import { useCallback, useEffect, useState } from "react";
import { getConfig, setConfig } from "./config-store";

/** Like useState, but the value is loaded from and saved to the app's
 * layout.json config file (survives closing/reopening the app). */
export function usePersistedValue<T>(key: string, defaultValue: T) {
  const [value, setValue] = useState(defaultValue);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    getConfig(key, defaultValue).then((v) => {
      setValue(v);
      setLoaded(true);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const update = useCallback(
    (next: T | ((prev: T) => T)) => {
      setValue((prev) => {
        const resolved = typeof next === "function" ? (next as (p: T) => T)(prev) : next;
        setConfig(key, resolved);
        return resolved;
      });
    },
    [key],
  );

  return [value, update, loaded] as const;
}

export const usePersistedNumber = usePersistedValue<number>;
