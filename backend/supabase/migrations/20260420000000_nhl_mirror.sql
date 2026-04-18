-- NHL mirror tables.
--
-- Background pollers populate these tables; every user-facing handler
-- reads from them. No NHL API calls happen in the request path after
-- this migration lands and handlers are rewritten.
--
-- See TECHNICAL-CACHING.md at the repo root for the full picture.

-- ---------------------------------------------------------------------
-- Games: schedule + state + score + series + venue, one row per game.
-- Live poller updates state/score/period; metadata poller refreshes
-- schedule for today and tomorrow every 5 min.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_games (
    game_id        BIGINT PRIMARY KEY,
    season         INTEGER NOT NULL,
    game_type      SMALLINT NOT NULL,
    game_date      DATE NOT NULL,
    start_time_utc TIMESTAMPTZ NOT NULL,
    game_state     TEXT NOT NULL,           -- FUT / PRE / LIVE / CRIT / OFF / FINAL
    home_team      TEXT NOT NULL,
    away_team      TEXT NOT NULL,
    home_score     INTEGER,
    away_score     INTEGER,
    period_number  SMALLINT,
    period_type    TEXT,
    series_status  JSONB,
    venue          TEXT,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_nhl_games_date  ON public.nhl_games (game_date);
CREATE INDEX IF NOT EXISTS idx_nhl_games_state ON public.nhl_games (game_state);

-- ---------------------------------------------------------------------
-- Per-player per-game stats. Distinct from playoff_skater_game_stats,
-- which is owned by the nightly ingest and used by the race-odds
-- projection model. This table must cover regular season too and must
-- be writable mid-game by the live poller.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_player_game_stats (
    game_id      BIGINT NOT NULL,
    player_id    BIGINT NOT NULL,
    team_abbrev  TEXT NOT NULL,
    position     TEXT NOT NULL,
    name         TEXT NOT NULL,
    goals        INTEGER NOT NULL DEFAULT 0,
    assists      INTEGER NOT NULL DEFAULT 0,
    points       INTEGER NOT NULL DEFAULT 0,
    sog          INTEGER,
    pim          INTEGER,
    plus_minus   INTEGER,
    hits         INTEGER,
    toi_seconds  INTEGER,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (game_id, player_id)
);

CREATE INDEX IF NOT EXISTS idx_npgs_player ON public.nhl_player_game_stats (player_id);
CREATE INDEX IF NOT EXISTS idx_npgs_team   ON public.nhl_player_game_stats (team_abbrev);

-- ---------------------------------------------------------------------
-- Skater season leaderboard mirror.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_skater_season_stats (
    player_id     BIGINT NOT NULL,
    season        INTEGER NOT NULL,
    game_type     SMALLINT NOT NULL,
    first_name    TEXT NOT NULL,
    last_name     TEXT NOT NULL,
    team_abbrev   TEXT NOT NULL,
    position      TEXT NOT NULL,
    goals         INTEGER NOT NULL DEFAULT 0,
    assists       INTEGER NOT NULL DEFAULT 0,
    points        INTEGER NOT NULL DEFAULT 0,
    plus_minus    INTEGER,
    faceoff_pct   REAL,
    toi_per_game  INTEGER,
    sog           INTEGER,
    headshot_url  TEXT,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (player_id, season, game_type)
);

CREATE INDEX IF NOT EXISTS idx_nsss_season_gt_points
    ON public.nhl_skater_season_stats (season, game_type, points DESC);

-- ---------------------------------------------------------------------
-- Goalie season mirror — narrow because we only consume a few fields.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_goalie_season_stats (
    player_id    BIGINT NOT NULL,
    season       INTEGER NOT NULL,
    game_type    SMALLINT NOT NULL,
    team_abbrev  TEXT NOT NULL,
    name         TEXT NOT NULL,
    record       TEXT,
    gaa          REAL,
    save_pctg    REAL,
    shutouts     INTEGER,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (player_id, season, game_type)
);

-- ---------------------------------------------------------------------
-- Team rosters. Replaces fetch_playoff_roster_pool's NHL fanout.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_team_rosters (
    team_abbrev  TEXT NOT NULL,
    season       INTEGER NOT NULL,
    roster       JSONB NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (team_abbrev, season)
);

-- ---------------------------------------------------------------------
-- Standings snapshot — one row per (season, team_abbrev).
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_standings (
    season         INTEGER NOT NULL,
    team_abbrev    TEXT NOT NULL,
    points         INTEGER NOT NULL,
    games_played   INTEGER NOT NULL,
    wins           INTEGER NOT NULL,
    losses         INTEGER NOT NULL,
    ot_losses      INTEGER NOT NULL,
    point_pctg     REAL,
    streak_code    TEXT,
    streak_count   INTEGER,
    l10_wins       INTEGER,
    l10_losses     INTEGER,
    l10_ot_losses  INTEGER,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (season, team_abbrev)
);

-- ---------------------------------------------------------------------
-- Playoff bracket snapshot. JSONB because the carousel is a hand-shaped
-- response we don't query into.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_playoff_bracket (
    season      INTEGER PRIMARY KEY,
    carousel    JSONB NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------
-- Pre-game landing matchup per game. Formalizes the insights_landing:*
-- response_cache rows into a typed table with write-once semantics:
-- captured_at is only set once per game_id, when the game is still FUT.
-- ---------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS public.nhl_game_landing (
    game_id     BIGINT PRIMARY KEY,
    matchup     JSONB NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------
-- View: today's running fantasy totals. Pulse's "points today" and the
-- daily rankings page both read from this view. Recomputes on every
-- read — safe at current scale (a few leagues, <30 teams each).
-- ---------------------------------------------------------------------

CREATE OR REPLACE VIEW public.v_daily_fantasy_totals AS
SELECT
    ft.league_id,
    ft.id       AS team_id,
    ft.name     AS team_name,
    g.game_date AS date,
    COALESCE(SUM(pgs.goals), 0)   AS goals,
    COALESCE(SUM(pgs.assists), 0) AS assists,
    COALESCE(SUM(pgs.points), 0)  AS points
FROM public.nhl_player_game_stats pgs
JOIN public.nhl_games g           ON g.game_id = pgs.game_id
JOIN public.fantasy_players fp    ON fp.nhl_id = pgs.player_id
JOIN public.fantasy_teams   ft    ON ft.id = fp.team_id
GROUP BY ft.league_id, ft.id, ft.name, g.game_date;
