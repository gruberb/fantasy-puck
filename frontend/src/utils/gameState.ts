// NHL `gameState` values: FUT/PRE (scheduled), LIVE/CRIT (in progress;
// CRIT is the final minutes of a close game), FINAL/OFF (done).

export function isLive(state: string | null | undefined): boolean {
  const s = (state ?? "").toUpperCase();
  return s === "LIVE" || s === "CRIT";
}

export function isFinal(state: string | null | undefined): boolean {
  const s = (state ?? "").toUpperCase();
  return s === "FINAL" || s === "OFF";
}
