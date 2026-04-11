-- Fix daily_rankings unique constraint to include league_id.
-- Production DB may have the old UNIQUE(team_id, date) from before league_id was added.
-- Drop it if it exists, then ensure the correct 3-column constraint is in place.

DO $$
BEGIN
    -- Drop old 2-column constraint if it exists (name varies by how it was created)
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.daily_rankings'::regclass
          AND contype = 'u'
          AND array_length(conkey, 1) = 2
    ) THEN
        EXECUTE format(
            'ALTER TABLE public.daily_rankings DROP CONSTRAINT %I',
            (SELECT conname FROM pg_constraint
             WHERE conrelid = 'public.daily_rankings'::regclass
               AND contype = 'u'
               AND array_length(conkey, 1) = 2
             LIMIT 1)
        );
    END IF;

    -- Add the correct 3-column constraint if it doesn't already exist
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.daily_rankings'::regclass
          AND contype = 'u'
          AND array_length(conkey, 1) = 3
    ) THEN
        ALTER TABLE public.daily_rankings
            ADD CONSTRAINT daily_rankings_team_id_date_league_id_key
            UNIQUE (team_id, date, league_id);
    END IF;
END $$;
