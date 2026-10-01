import { useCallback, useEffect, useRef, useState } from "react";

export type FlashTone = "success" | "error";

export interface Flash {
  message: string;
  tone: FlashTone;
}

const DEFAULT_DURATION_MS: Record<FlashTone, number> = {
  success: 4000,
  error: 6000,
};

/**
 * One transient message slot for `<Toast>`. A new flash replaces the
 * current one and restarts the timer, so a stale timeout can never
 * dismiss a newer message early.
 */
export function useFlash() {
  const [flash, setFlash] = useState<Flash | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const showFlash = useCallback(
    (message: string, tone: FlashTone = "success", durationMs = DEFAULT_DURATION_MS[tone]) => {
      clearTimeout(timer.current);
      setFlash({ message, tone });
      timer.current = setTimeout(() => setFlash(null), durationMs);
    },
    [],
  );

  useEffect(() => () => clearTimeout(timer.current), []);

  return { flash, showFlash };
}
