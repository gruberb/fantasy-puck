-- Aggregated 5-year playoff skater totals.
--
-- One row per player (natural key: name + year of birth, which
-- disambiguates the real-world duplicate "Sebastian Aho" without needing
-- an NHL player_id). Populated once at startup from a CSV bundled into
-- the binary; see backend/src/utils/historical_seed.rs.
--
-- Used as a shrinkage prior in the Bayesian player-projection blend —
-- gives the model a regression-to-mean anchor for every rostered skater
-- who has appeared in a recent playoff, especially early in the round
-- when current-playoff samples are tiny.

CREATE TABLE IF NOT EXISTS public.historical_playoff_skater_totals (
    player_name  TEXT NOT NULL,
    born         INTEGER NOT NULL,
    team         TEXT NOT NULL,         -- most recent team or "TOT"
    position     TEXT NOT NULL,
    gp           INTEGER NOT NULL,
    g            INTEGER NOT NULL DEFAULT 0,
    a            INTEGER NOT NULL DEFAULT 0,
    p            INTEGER NOT NULL DEFAULT 0,
    shots        INTEGER,
    toi_seconds  INTEGER,
    PRIMARY KEY (player_name, born)
);

-- Name-only lookup for the common case: the projection module resolves a
-- current fantasy roster by name (and falls back to full key only on
-- collision).
CREATE INDEX IF NOT EXISTS idx_historical_playoff_skater_totals_name
    ON public.historical_playoff_skater_totals (player_name);
