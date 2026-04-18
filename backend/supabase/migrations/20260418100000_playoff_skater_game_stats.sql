-- Per-player per-game playoff facts.
--
-- One row per (game_id, player_id). Populated by the nightly ingest job
-- (backend/src/utils/scheduler.rs) from completed playoff box scores, and
-- by the one-shot backfill for games that happened before the ingest job
-- existed.
--
-- Feeds the Bayesian player-projection blend (rolling form + full-playoff
-- rate) and, eventually, a playoff-Elo team-strength update loop.

CREATE TABLE IF NOT EXISTS public.playoff_skater_game_stats (
    season       INTEGER NOT NULL,
    game_type    SMALLINT NOT NULL,       -- 3 = playoffs (only value today)
    game_id      BIGINT NOT NULL,
    game_date    DATE NOT NULL,
    player_id    BIGINT NOT NULL,
    team_abbrev  TEXT NOT NULL,
    opponent     TEXT NOT NULL,
    home         BOOLEAN NOT NULL,
    goals        INTEGER NOT NULL DEFAULT 0,
    assists      INTEGER NOT NULL DEFAULT 0,
    points       INTEGER NOT NULL DEFAULT 0,
    shots        INTEGER,
    pp_points    INTEGER,
    toi_seconds  INTEGER,
    PRIMARY KEY (game_id, player_id)
);

-- Fast "last N games for this player" lookups — the primary read pattern
-- for the recency-weighted term of the projection blend.
CREATE INDEX IF NOT EXISTS idx_playoff_skater_game_stats_player_date
    ON public.playoff_skater_game_stats (player_id, game_date DESC);

-- Fast "all players on this team for this game" lookups — used by the
-- backfill and ingest paths when stepping through a box-score response.
CREATE INDEX IF NOT EXISTS idx_playoff_skater_game_stats_team_game
    ON public.playoff_skater_game_stats (team_abbrev, game_id);

-- Fast "all playoff games on date X" lookups — used by the backtest
-- harness when reconstructing state as-of a historical day.
CREATE INDEX IF NOT EXISTS idx_playoff_skater_game_stats_date
    ON public.playoff_skater_game_stats (game_date);
