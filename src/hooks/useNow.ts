import { useEffect, useState } from "react";

/**
 * The current time in milliseconds, refreshed every `intervalMs` while
 * `active` is true. While inactive it stays put, so a screen with nothing
 * running does not re-render once a second.
 *
 * Right after `active` turns true the value can be up to one interval old;
 * callers that subtract it from a start time should clamp at zero.
 */
export function useNow(active: boolean, intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [active, intervalMs]);
  return now;
}
