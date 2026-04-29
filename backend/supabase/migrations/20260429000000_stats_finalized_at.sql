-- Mark the moment a game's per-player boxscore is post-buzzer-synced.
--
-- Until this column is set the row is treated as "still moving": the live
-- poller may rewrite it on the next tick, and aggregated surfaces (overall
-- rankings, top-rostered, NHL-teams-we-roster, team detail Total Points,
-- sleeper-pick) exclude it from sums. The poller stamps NOW() once it has
-- pulled one final boxscore after FINAL/OFF + a 15-minute grace window so
-- post-buzzer scoring corrections (empty-net assist credit, OT/shootout
-- settlement, late official changes) land before the row is sealed.
--
-- Why a column on `nhl_games` rather than per-row on `nhl_player_game_stats`:
-- finalisation is a property of the game, not the row. One UPDATE per game
-- vs. N UPDATEs per game; one filter join vs. a per-row predicate.

ALTER TABLE public.nhl_games
    ADD COLUMN IF NOT EXISTS stats_finalized_at TIMESTAMPTZ NULL;

-- Partial index for the live poller's "needs final-sync" sweep. Hits only
-- the tiny set of rows whose game has ended but whose boxscore hasn't been
-- sealed yet. Excludes the bulk of historical games where the column is
-- already populated.
CREATE INDEX IF NOT EXISTS idx_nhl_games_needs_final_sync
    ON public.nhl_games (game_state, updated_at)
    WHERE stats_finalized_at IS NULL;

-- One-shot backfill so already-completed games render on the dashboard
-- immediately after this migration deploys. Without this, every
-- aggregated surface that filters on `stats_finalized_at IS NOT NULL`
-- would show zero points until either the live poller's final-sync pass
-- works through every historical game (slow — one boxscore HTTP call
-- each) or an operator runs `/api/admin/rehydrate` (also slow). The
-- backfill stamps `updated_at` rather than NOW() so the timestamp at
-- least roughly reflects when the row was last considered fresh; it
-- does NOT refetch the boxscore, so any pre-existing drift survives
-- until rehydrate is run for the playoff date range.
UPDATE public.nhl_games
   SET stats_finalized_at = updated_at
 WHERE game_state IN ('FINAL', 'OFF')
   AND stats_finalized_at IS NULL;
