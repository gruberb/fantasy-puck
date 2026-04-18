-- Team-level result per completed playoff game.
--
-- One row per game_id. Populated by the nightly ingest alongside the
-- per-skater table, so a chronological `ORDER BY game_date, game_id`
-- replays every playoff game in sequence — exactly what the playoff Elo
-- update loop needs.
--
-- Kept separate from playoff_skater_game_stats to avoid joining and
-- aggregating 20+ skater rows back into one team row on every Elo read.

CREATE TABLE IF NOT EXISTS public.playoff_game_results (
    season       INTEGER NOT NULL,
    game_type    SMALLINT NOT NULL,
    game_id      BIGINT PRIMARY KEY,
    game_date    DATE NOT NULL,
    home_team    TEXT NOT NULL,
    away_team    TEXT NOT NULL,
    home_score   INTEGER NOT NULL,
    away_score   INTEGER NOT NULL,
    winner       TEXT NOT NULL,          -- winning team's abbrev
    round        SMALLINT                -- 1-4; NULL if unknown
);

-- Chronological replay index. Elo iterates in game-order.
CREATE INDEX IF NOT EXISTS idx_playoff_game_results_chronological
    ON public.playoff_game_results (game_date, game_id);

-- Team-scoped lookups (last N games, per-team goal differential, etc.).
CREATE INDEX IF NOT EXISTS idx_playoff_game_results_home
    ON public.playoff_game_results (home_team, game_date);
CREATE INDEX IF NOT EXISTS idx_playoff_game_results_away
    ON public.playoff_game_results (away_team, game_date);
