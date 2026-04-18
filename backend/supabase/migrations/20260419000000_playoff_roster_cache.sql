-- Playoff team roster pool, cached at prewarm time.
--
-- The player pool for the draft and the stats page is built from the
-- rosters of every team currently in the playoff bracket (16 teams).
-- Fetching those rosters in parallel on every cold request bursts the
-- NHL API hard enough to trip the rate limit, especially during playoff
-- evenings when live-game and rate-odds traffic is already pressing.
--
-- Cache shape: a single JSONB blob per (season, game_type) carrying the
-- full PoolMap (player_id -> name/position/team/headshot). The daily
-- 10:00 UTC prewarm refreshes it; reads are one row per request.

CREATE TABLE IF NOT EXISTS public.playoff_roster_cache (
    season       INTEGER NOT NULL,
    game_type    SMALLINT NOT NULL,
    rosters      JSONB NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (season, game_type)
);
