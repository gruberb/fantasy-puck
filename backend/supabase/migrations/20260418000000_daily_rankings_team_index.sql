-- Add a composite index to speed up the Pulse/Insights sparkline queries
-- that pull last-N-days of points for a single team in a league.
CREATE INDEX IF NOT EXISTS idx_daily_rankings_team_league_date
    ON public.daily_rankings(team_id, league_id, date DESC);
