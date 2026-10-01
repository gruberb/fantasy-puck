-- Backstop for the row lock that serializes draft picks: one pick per
-- slot and one pick per player within a draft session.
--
-- Migrations run at boot, so an index build that fails on existing
-- duplicates would take the server down. Each index is created only when
-- the data already satisfies it; otherwise a NOTICE is logged and the
-- row lock alone keeps new picks consistent.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.draft_picks
         GROUP BY draft_session_id, pick_number HAVING COUNT(*) > 1
    ) THEN
        CREATE UNIQUE INDEX IF NOT EXISTS idx_draft_picks_session_pick
            ON public.draft_picks (draft_session_id, pick_number);
    ELSE
        RAISE NOTICE 'draft_picks has duplicate (draft_session_id, pick_number); unique index skipped';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM public.draft_picks
         GROUP BY draft_session_id, nhl_id HAVING COUNT(*) > 1
    ) THEN
        CREATE UNIQUE INDEX IF NOT EXISTS idx_draft_picks_session_player
            ON public.draft_picks (draft_session_id, nhl_id);
    ELSE
        RAISE NOTICE 'draft_picks has duplicate (draft_session_id, nhl_id); unique index skipped';
    END IF;
END $$;
