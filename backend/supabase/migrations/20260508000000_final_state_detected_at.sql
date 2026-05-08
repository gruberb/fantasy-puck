-- Track the first time the mirror observes a game in FINAL/OFF.
--
-- `nhl_games.updated_at` is a generic freshness marker and the meta poller
-- refreshes final schedules every few minutes. The final-sync grace window
-- must not depend on that column, otherwise a completed game's player stats
-- can remain unsealed while every schedule refresh moves `updated_at` forward.

ALTER TABLE public.nhl_games
    ADD COLUMN IF NOT EXISTS final_state_detected_at TIMESTAMPTZ NULL;

UPDATE public.nhl_games
   SET final_state_detected_at = COALESCE(
       (
           SELECT MAX(pgs.updated_at)
             FROM public.nhl_player_game_stats pgs
            WHERE pgs.game_id = nhl_games.game_id
       ),
       updated_at
   )
 WHERE game_state IN ('FINAL', 'OFF')
   AND final_state_detected_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_nhl_games_needs_final_sync_v2
    ON public.nhl_games (game_state, final_state_detected_at)
    WHERE stats_finalized_at IS NULL;
