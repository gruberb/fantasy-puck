-- Fields the race-odds model reads that the mirror used to drop, so the
-- request path can stop calling the NHL API live.
--
-- nhl_standings.raw: the full NHL standings entry. Elo seeding and the
-- per-team home-ice bonus read home/road splits and other fields that
-- the typed columns don't carry.
-- nhl_goalie_season_stats.wins: the "primary starter" signal for the
-- goalie rating bonus.

ALTER TABLE public.nhl_standings ADD COLUMN IF NOT EXISTS raw JSONB;
ALTER TABLE public.nhl_goalie_season_stats ADD COLUMN IF NOT EXISTS wins INTEGER;
